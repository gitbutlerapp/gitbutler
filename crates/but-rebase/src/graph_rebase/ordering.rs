#![doc = include_str!("../../docs/commit_parentage.md")]

use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use but_core::RefMetadata;

use crate::graph_rebase::{CommitIndex, Editor, util};

impl<M: RefMetadata> Editor<'_, M> {
    /// Order commit indices by parentage, with parents first and children last.
    ///
    /// Duplicate commits are deduplicated by commit-id with first occurrence winning.
    ///
    /// Ordering is derived from a deterministic rank map built from the editor store.
    /// The rank is computed by traversing from child-most graph entries in ordered-parent
    /// post-order (parents are pushed in `collect_ordered_parents` order, without reversing),
    /// then sorting selected commits by `(rank, input_order)`.
    ///
    /// Traversal starts at the editor's own `HEAD` checkout, then covers the remaining
    /// child-most entries: commits from the entrypoint's history rank before commits only an
    /// auxiliary region (a linked worktree's branch) reaches, however the rows happen to be
    /// laid out.
    ///
    /// The ranker considers only selected commit ids and exits traversal early once all selected
    /// commits have been ranked.
    pub fn order_by_parentage<I>(&self, commits: I) -> Result<Vec<CommitIndex>>
    where
        I: IntoIterator<Item = CommitIndex>,
    {
        // Normalize user input to unique commits while retaining first-seen order for tie-breaking.
        let mut selected = Vec::<SelectedCommit>::new();
        let mut seen_ids = HashSet::<gix::ObjectId>::new();
        for (input_order, commit_ix) in commits.into_iter().enumerate() {
            let commit = self.commit_of(commit_ix)?;
            if seen_ids.insert(commit.id) {
                selected.push(SelectedCommit {
                    commit: commit_ix,
                    id: commit.id,
                    input_order,
                });
            }
        }

        if selected.len() <= 1 {
            return Ok(selected.into_iter().map(|s| s.commit).collect());
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
                    "Cannot order selected commits by parentage: selected commit {} could not be ranked from editor store entries",
                    commit.id
                );
            }
        }

        // The rank map is the sole source of truth for deterministic parent-before-child ordering.
        selected.sort_by_key(|commit| (graph_rank[&commit.id], commit.input_order));

        Ok(selected.into_iter().map(|s| s.commit).collect())
    }
}

#[derive(Debug, Clone, Copy)]
struct SelectedCommit {
    commit: CommitIndex,
    id: gix::ObjectId,
    input_order: usize,
}

fn parent_to_child_rank<M: RefMetadata>(
    editor: &Editor<'_, M>,
    selected_ids: &HashSet<gix::ObjectId>,
) -> Result<HashMap<gix::ObjectId, usize>> {
    let mut rank_by_id = HashMap::<gix::ObjectId, usize>::new();
    let mut next_rank = 0usize;
    let mut seen = HashSet::<CommitIndex>::new();

    let mut roots = editor.store.commits.tips().collect::<Vec<CommitIndex>>();
    roots.sort_unstable();
    // The entrypoint's region ranks first: its history is what the user is looking at, and
    // auxiliary regions (linked worktrees) come after it — row order must not decide.
    if let Some(head_pick) = editor.checkouts.iter().find_map(|checkout| match checkout {
        crate::graph_rebase::Checkout::Head { entry, .. } => editor.store.resolve_to_commit(*entry),
        crate::graph_rebase::Checkout::Worktree { .. } => None,
    }) {
        roots.retain(|&root| root != head_pick);
        roots.insert(0, head_pick);
    }

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
                if let Some(id) = editor.store.commit_id(entry)
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

            let parents = util::collect_ordered_parents(&editor.store, entry);
            stack.push((entry, true));
            for parent_idx in parents.into_iter() {
                stack.push((parent_idx, false));
            }
        }
    }

    Ok(rank_by_id)
}
