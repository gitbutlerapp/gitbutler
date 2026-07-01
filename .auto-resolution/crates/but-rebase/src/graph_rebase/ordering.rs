#![doc = include_str!("../../docs/commit_parentage.md")]

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use but_core::RefMetadata;
use petgraph::Direction;

use crate::graph_rebase::{Editor, Pick, Selector, Step, StepGraphIndex, ToCommitSelector, util};

impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Order commit selectors by parentage, with parents first and children last.
    ///
    /// Duplicate selectors are deduplicated by commit-id with first occurrence winning.
    ///
    /// Ordering is derived from a deterministic rank map built from the editor step graph.
    /// The rank is computed by traversing from all child-most graph nodes in ordered-parent
    /// post-order (parents are pushed in `collect_ordered_parents` order, without reversing),
    /// then sorting selected commits by `(rank, input_order)`.
    ///
    /// The ranker considers only selected commit ids and exits traversal early once all selected
    /// commits have been ranked.
    pub fn order_commit_selectors_by_parentage<I, S>(&self, selectors: I) -> Result<Vec<Selector>>
    where
        I: IntoIterator<Item = S>,
        S: ToCommitSelector,
    {
        // Normalize user input to unique commits while retaining first-seen order for tie-breaking.
        let mut selected = Vec::<SelectedCommit>::new();
        let mut seen_ids = HashSet::<gix::ObjectId>::new();
        for (input_order, selector_like) in selectors.into_iter().enumerate() {
            let (selector, commit) = self.find_selectable_commit(selector_like)?;
            if seen_ids.insert(commit.id) {
                selected.push(SelectedCommit {
                    selector,
                    id: commit.id,
                    input_order,
                });
            }
        }

        if selected.len() <= 1 {
            return Ok(selected.into_iter().map(|s| s.selector).collect());
        }

        // Build a deterministic rank from editor step-graph order.
        let selected_ids = selected
            .iter()
            .map(|commit| commit.id)
            .collect::<HashSet<_>>();
        let step_graph_rank = step_graph_parent_to_child_rank(self, &selected_ids)?;

        // Preserve the Result contract: unreachable selected commits are a runtime error,
        // not an internal panic.
        for commit in &selected {
            if !step_graph_rank.contains_key(&commit.id) {
                bail!(
                    "Cannot order selected commits by parentage: selected commit {} could not be ranked from editor graph nodes",
                    commit.id
                );
            }
        }

        // The rank map is the sole source of truth for deterministic parent-before-child ordering.
        selected.sort_by_key(|commit| {
            let rank = step_graph_rank
                .get(&commit.id)
                .copied()
                .unwrap_or(usize::MAX);
            (rank, commit.input_order)
        });

        Ok(selected.into_iter().map(|s| s.selector).collect())
    }
}

#[derive(Debug, Clone, Copy)]
struct SelectedCommit {
    selector: Selector,
    id: gix::ObjectId,
    input_order: usize,
}

fn step_graph_parent_to_child_rank<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selected_ids: &HashSet<gix::ObjectId>,
) -> Result<HashMap<gix::ObjectId, usize>> {
    let mut rank_by_id = HashMap::<gix::ObjectId, usize>::new();
    let mut next_rank = 0usize;
    let mut seen = HashSet::<StepGraphIndex>::new();

    let mut roots = editor
        .graph
        .externals(Direction::Incoming)
        .collect::<Vec<StepGraphIndex>>();
    roots.sort_unstable_by_key(|idx| idx.index());

    // Traverse from all child-most entrypoints (graph nodes without children), assigning
    // rank in post-order so parent commits always rank before descendants. Parents are
    // pushed in collect_ordered_parents order (not reversed). The seen-set handles nodes
    // reachable from multiple entrypoints, and traversal stops once all selected commits
    // have ranks.
    for root in roots {
        if rank_by_id.len() == selected_ids.len() {
            break;
        }

        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            if rank_by_id.len() == selected_ids.len() {
                break;
            }

            if expanded {
                if let Step::Pick(Pick { id, .. }) = editor.graph[node]
                    && selected_ids.contains(&id)
                {
                    rank_by_id.entry(id).or_insert_with(|| {
                        let rank = next_rank;
                        next_rank += 1;
                        rank
                    });
                }
                continue;
            }

            if !seen.insert(node) {
                continue;
            }

            let parents = util::collect_ordered_parents(&editor.graph, node);
            stack.push((node, true));
            for parent_idx in parents.into_iter() {
                stack.push((parent_idx, false));
            }
        }
    }

    Ok(rank_by_id)
}
