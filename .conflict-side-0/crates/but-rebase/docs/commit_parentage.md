# Commit Parentage Ordering

This document explains how commit selector ordering works in
`Editor::order_commit_selectors_by_parentage` (implemented in
`but-rebase/src/graph_rebase/ordering.rs`), and why the implementation is
structured the way it is.

## Goal

Given a set of commit selectors, produce a deterministic order such that:

- Parent commits come before child commits.
- Unrelated commits still have a stable, deterministic order.
- Duplicate selectors are deduplicated by commit id (first occurrence wins).

This ordering is useful for operations that must apply commits in dependency-safe order.

## Inputs and Output

Method: `editor.order_commit_selectors_by_parentage(selectors) -> Result<Vec<Selector>>`

- Input selectors can be any type implementing `ToCommitSelector`.
- The output is a list of normalized `Selector` values.

## Preconditions and Errors

The function treats the editor step graph as the single source of truth.

This means:

- Commits must be selectable in the editor graph.
- If a commit is absent from the editor graph, selector resolution fails (for example: `Failed to find commit <oid> in rebase editor`).
- No workspace projection lookup is used for ordering.

## High-Level Pipeline

The current algorithm has three phases.

1. Normalize and deduplicate input
- Resolve each incoming selector to `(Selector, CommitOwned)` with `editor.find_selectable_commit`.
- Keep only the first occurrence of each commit id.
- Preserve `input_order` for deterministic tie-breaking.

2. Build rank map from editor traversal

Build a map `commit_id -> rank` by traversing the entire editor step graph from all child-most nodes.

Implementation notes:

- Seed traversal with `editor.graph.externals(Direction::Incoming)`, i.e. all nodes with no children.
- Sort these traversal entrypoints by graph index for deterministic iteration.
- Traverse parent-direction edges using `collect_ordered_parents`, which respects parent edge weights.
- Push parents onto the traversal stack in that same order (no reversal).
- Use iterative DFS with post-order assignment so parents are ranked before descendants.
- Use a global `seen` set so overlapping traversals do not revisit or re-rank nodes seen earlier.
- Consider only selected commit ids (non-selected picks are ignored).
- Stop traversal early once all selected commit ids have a rank.

3. Sort selected commits by rank
- Sort selected commits by `(rank, input_order)`.
- `rank` is the sole ancestry source of truth for ordering.
- `input_order` only breaks ties if equal ranks appear.

## Example

Given a graph

```sh
*-.   1348870 (HEAD -> main) Merge branches 'A', 'B' and 'C'
|\ \
| | * 930563a (C) C: add another 10 lines to new file
| | * 68a2fc3 C: add 10 lines to new file
| | * 984fd1c C: new file with 10 lines
| * | a748762 (B) B: another 10 lines at the bottom
| * | 62e05ba B: 10 lines at the bottom
| |/
* / add59d2 (A) A: 10 lines on top
|/
* 8f0d338 (tag: base) base
```

And the user asks to order selectors in this input sequence:

- `a748762`
- `add59d2`
- `62e05ba`

The ranked would produce

- `8f0d338` (`base`) = 0
- `add59d2` (`A`) = 1
- `62e05ba` (`B~`) = 2
- `a748762` (`B`) = 3
- `984fd1c` (`C~2`) = 4
- `68a2fc3` (`C~`) = 5
- `930563a` (`C`) = 6
- STOP

The rank is assigned in post-order while traversing from all child-most nodes toward parents.
This means all nodes in the editor graph are eligible for ranking, not only those reachable from checkout roots.

Then sorting by `(rank, input_order)` returns:

- `add59d2` (rank 1)
- `62e05ba` (rank 2)
- `a748762` (rank 3)

If two commits had the same rank, the one that appeared earlier in input would come first.

## Complexity

Let `n` be number of selected unique commits.

- Ranking traversal is linear in the visited subgraph: `O(V + E)`.
- Sorting selected commits is `O(n log n)`.
- With early-exit ranking, traversal can end before visiting the full graph when selected commits are found early.

## Determinism Guarantees

Determinism is achieved by:

- deduping by first occurrence,
- deriving rank from ordered-parent traversal,
- using `input_order` as secondary tiebreaker.

So repeated runs with the same inputs and editor graph state produce the same output.

## Notes for Future Changes

Ordering intentionally operates only on commits represented in the editor graph.
If behavior needs to include commits not represented there, that must be solved before ordering
(for example by changing editor graph construction).
