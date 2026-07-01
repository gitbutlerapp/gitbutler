# Merge Commit Changes

This module contains the editor helpers for combining selected commit changes
into a single tree and for planning those changes as mergeable `base..commit`
ranges.

## `merge_commit_changes_to_tree`

`Editor::merge_commit_changes_to_tree(target, subjects, merge_options)` builds a
tree by:

1. using the target commit's full visible tree as the baseline,
2. asking the planner to normalize the selected subject commits into mergeable
   change ranges,
3. merging those planned `base_tree_id -> commit^{tree}` ranges into the
   accumulated tree in planner-defined order, and
4. stopping at the first unresolved merge conflict while returning the
   auto-resolved tree plus conflict metadata.

The target contributes its whole tree. Subject commits contribute only their
selected change ranges.

## `plan_commit_changes_for_merge`

`Editor::plan_commit_changes_for_merge(target, subjects)` turns a set of
selected subject commits into emitted `PlannedCommitChange` entries.

The planner uses a single traversal of the editor graph, rooted from the same
child-most entrypoints used by commit parentage ordering. During that traversal
it derives:

- canonical subject order from editor parentage,
- selected first-parent relationships for contiguous-chain collapse, and
- the target ancestry cone for pruning.

The planner then emits only non-pruned selected-chain tips.

Even though traversal and pruning are driven by the editor step graph, the
planner still takes first-parent and tree semantics from the commit objects in
the editor's in-memory repository. In other words:

- the in-memory repository is the source of truth for commit parents, trees,
  and SHAs,
- the step graph is used only to walk the selected region deterministically and
  to discover the target ancestry cone quickly, and
- callers are expected to keep those two views aligned before using these
  helpers.

## Design Decisions

- The planner is the sole source of truth for subject ordering. Call-sites
  should pass raw selected subject IDs and not pre-order them.
- Subject ordering is canonicalized from the editor graph, not from caller
  input order.
- Duplicate subject IDs are deduplicated by commit ID only.
- Pruning uses full target ancestry across any parent edge.
- Collapse uses only selected first-parent chains.
- Pruned commits are never emitted as merge picks.
- A pruned selected first parent may still define the `base_tree_id` boundary
  for a surviving descendant, so a surviving tip can emit `B..C` even when `B`
  is pruned.
- The editor step graph is expected to represent the same topology as the
  editor's in-memory repository. These helpers do not treat step-graph rewiring
  as a new source of commit parent truth.
- The editor is assumed to already be normalized and up to date before calling
  these helpers. Callers chaining earlier editor mutations should normalize via
  `editor.rebase()?.into_editor()` first.
