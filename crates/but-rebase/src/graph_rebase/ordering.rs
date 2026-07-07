#![doc = include_str!("../../docs/commit_parentage.md")]

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use but_core::RefMetadata;

use crate::graph_rebase::{Editor, EditorGraphIndex, Selector, ToCommitSelector, util};

impl<M: RefMetadata> Editor<'_, M> {
    /// Order commit selectors by parentage, with parents first and children last.
    ///
    /// Duplicate selectors are deduplicated by commit-id with first occurrence winning.
    ///
    /// Ordering is derived from a deterministic rank map built from the editor graph.
    /// The rank is computed by traversing from all child-most graph entries in ordered-parent
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

        // Build a deterministic rank from editor commit-graph order.
        let selected_ids = selected
            .iter()
            .map(|commit| commit.id)
            .collect::<HashSet<_>>();
        let graph_rank = parent_to_child_rank(self, &selected_ids)?;

        // Preserve the Result contract: unreachable selected commits are a runtime error,
        // not an internal panic.
        for commit in &selected {
            if !graph_rank.contains_key(&commit.id) {
                bail!(
                    "Cannot order selected commits by parentage: selected commit {} could not be ranked from editor graph entries",
                    commit.id
                );
            }
        }

        // The rank map is the sole source of truth for deterministic parent-before-child ordering.
        selected.sort_by_key(|commit| (graph_rank[&commit.id], commit.input_order));

        Ok(selected.into_iter().map(|s| s.selector).collect())
    }
}

#[derive(Debug, Clone, Copy)]
struct SelectedCommit {
    selector: Selector,
    id: gix::ObjectId,
    input_order: usize,
}

fn parent_to_child_rank<M: RefMetadata>(
    editor: &Editor<'_, M>,
    selected_ids: &HashSet<gix::ObjectId>,
) -> Result<HashMap<gix::ObjectId, usize>> {
    let mut rank_by_id = HashMap::<gix::ObjectId, usize>::new();
    let mut next_rank = 0usize;
    let mut seen = HashSet::<EditorGraphIndex>::new();

    let mut roots = editor.graph.tips().collect::<Vec<EditorGraphIndex>>();
    roots.sort_unstable();

    // Traverse from all child-most entrypoints (graph entries without children), assigning
    // rank in post-order so parent commits always rank before descendants. Parents are
    // pushed in collect_ordered_parents order (not reversed). The seen-set handles entries
    // reachable from multiple entrypoints, and traversal stops once all selected commits
    // have ranks.
    for root in roots {
        if rank_by_id.len() == selected_ids.len() {
            break;
        }

        let mut stack = vec![(root, false)];
        while let Some((entry, expanded)) = stack.pop() {
            if rank_by_id.len() == selected_ids.len() {
                break;
            }

            if expanded {
                if let Some(id) = editor.graph.commit_id(entry)
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

            if !seen.insert(entry) {
                continue;
            }

            let parents = util::collect_ordered_parents(&editor.graph, entry);
            stack.push((entry, true));
            for parent_idx in parents.into_iter() {
                stack.push((parent_idx, false));
            }
        }
    }

    Ok(rank_by_id)
}
