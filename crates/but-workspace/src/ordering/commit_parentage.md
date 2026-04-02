# Commit Parentage Ordering

This document explains how commit selector ordering works in [commit_parentage.rs](commit_parentage.rs), and why the implementation is structured the way it is.

## Goal

Given a set of commit selectors, produce a deterministic order such that:

- Parent commits come before child commits.
- Unrelated commits still have a stable, deterministic order.
- Duplicate selectors are deduplicated by commit id (first occurrence wins).

This ordering is useful for operations that must apply commits in dependency-safe order.

## Inputs and Output

Function: `order_commit_selectors_by_parentage(editor, selectors) -> Result<Vec<Selector>>`

- Input selectors can be any type implementing `ToCommitSelector`.
- The output is a list of normalized `Selector` values.

## Preconditions and Errors

The function returns an error if a selected commit cannot be found in the workspace traversal represented by `editor.workspace`.

Why this is required:

- Deterministic tie-breaking depends on workspace traversal rank.
- Segment-based ancestry checks also depend on the workspace graph/projection.

## High-Level Pipeline

The algorithm has five phases.

1. Normalize and deduplicate input
- Resolve each incoming selector to `(Selector, CommitOwned)` with `editor.find_selectable_commit`.
- Keep only the first occurrence of each commit id.
- Resolve and store each commit's owning `SegmentIndex`.

2. Compute deterministic fallback rank
- Build a map: `commit_id -> rank` from workspace parent-to-child traversal order.
- This rank is used only when ancestry does not constrain order.

3. Build ancestry constraint graph
- For every selected pair `(left, right)`, determine relation.
- If `left` is ancestor of `right`, add directed edge `left -> right`.
- If `right` is ancestor of `left`, add directed edge `right -> left`.
- If unrelated, add no edge.

4. Topological sort with stable tie-breaking
- Use Kahn's algorithm over indegrees.
- Keep all currently ready nodes in a min-priority structure keyed by:
  - `(workspace_rank, input_order)`
- Repeatedly pop the best ready node, emit it, and reduce indegree of its children.

5. Validate completeness
- If output length is smaller than selected length, constraints were cyclic/inconsistent.
- Return an explicit error in that case.

## How Ancestry Is Determined

The implementation prefers segment-level relation checks first, then falls back to commit-level merge-base logic only when needed.

### Segment-first classification

For selected commits `left` and `right`, call:

- `editor.workspace.graph.relation_between(left.segment_id, right.segment_id)`

Mapping used:

- `Ancestor` -> `LeftIsAncestorOfRight`
- `Descendant` -> `RightIsAncestorOfLeft`
- `Disjoint` or `Diverged` -> `Unrelated`
- `Identity` -> unresolved at segment level, so use commit-level fallback

### Same-segment fallback

When both commits are in the same segment (`Identity`), they can still have parent-child relation. In that case:

- Compute merge-base on commit ids.
- If merge-base is `left`, then `left` is ancestor of `right`.
- If merge-base is `right`, then `right` is ancestor of `left`.
- Otherwise, treat as unrelated.

This hybrid approach keeps common cases cheap and explicit while preserving correctness inside a single segment.

## Why Not Pure Commit Merge-Base For Everything?

Pure commit-level checks for every pair work, but they are less explicit about workspace/segment intent and duplicate logic now captured in `Graph::relation_between`.

Using segment relations first gives:

- clearer semantics aligned with the graph model,
- faster short-circuiting for many pairs,
- one shared place for relationship semantics.

## Complexity

Let `n` be number of selected unique commits.

- Pairwise relation discovery: `O(n^2)` comparisons.
- Topological processing:
  - each push/pop on ready queue: `O(log n)`
  - overall typically `O((n + e) log n)` where `e` is number of ancestry edges.

Total dominated by pairwise relation checks plus heap operations.

## Determinism Guarantees

Determinism is achieved by:

- deduping by first occurrence,
- using workspace rank for unrelated commits,
- using `input_order` as secondary tiebreaker.

So repeated runs with the same inputs and workspace state produce the same output.

## Notes for Future Changes

If behavior needs to tolerate commits not present in workspace traversal, one possible policy is:

- assign such commits rank after all ranked commits,
- preserve relative order by `input_order`.

Current implementation intentionally errors to keep assumptions strict and explicit.
