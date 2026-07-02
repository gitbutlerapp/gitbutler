# Intentional graph-behavior changes (CommitGraph flip)

> **STATUS 2026-07-02 — PROJECTION PARITY COMPLETE.** The `BUT_GRAPH_PARITY`
> sweep (a temporary panic inside `from_commit_traversal` comparing the walk's
> and the flip's `into_workspace()` fingerprints) passes on the FULL but-graph
> test suite: **0 divergences across 196 tests** — every `from_head` /
> `from_commit_traversal` call in the suite, including entrypoint, extra-target,
> limit, and detached states. The items below describe *structural* (graph-tree)
> differences that remain by design; the projection they feed is identical.
> See "Execution readiness" at the bottom for the deletion plan.

As part of replacing the SegmentGraph walk with the CommitGraph-derived build, we
are **deliberately simplifying** several graph-shaping behaviors. These are not
bugs in the flip — they are decisions to drop walk behaviors that add complexity
without clear value. Recorded here for the eventual commit message / PR body.

## 1. No duplicate parents in workspace merge commits
The GitButler workspace merge previously encoded empty lanes as **repeated
parents** (e.g. `[base, base]`). We abandon that as a *design* idea: lanes are
derived from workspace metadata, not the parent array. Both the walk and the
flip collapse exact duplicate parents at read time
(`CommitGraph::from_repository_seeded`: `[X,X]→[X]`, keeping `[X,Y]` and
`[X,Y,X]→[X,Y]`), so a merge with repeated parents renders as if it had the
distinct set.
- **Effect:** no visible change to the rendered graph.
- **Tests:** `duplicate_parent_connection_from_ws_commit_to_ambiguous_branch`
  and its `_no_advanced_target` sibling KEEP their fixtures with duplicated
  parents and now assert the *deduped* outcome (the branch is visible once).
  They stay as defensive coverage that dup-parent commits in the wild are
  handled — they are not the abandoned "two lanes from dup parents" behavior.

## 2. The entrypoint segment is named, not forced anonymous
When checked out *into* a stack (e.g. on branch `lane`), the walk forced the
entrypoint segment **anonymous** and floated the branch as a separate empty
segment above it. We drop that: the checked-out branch simply names the segment
it sits on.
- **Before:** `anon:` holding the commit + a floating empty `👉lane` above it.
- **After:** `👉lane` names the commit's segment directly.
- Note: a *truly detached* HEAD (no ref) stays anonymous — there is no branch to
  name it. Only the "checked-out branch" case changes.

## 3. No separate empty segment for the target's local branch
The walk emitted a floating empty segment for the target's local branch (`main`)
purely to mark where the target rests (`ensure_local_tracking_segment_for_remote`).
We drop it: `main` is a plain ref on the commit, and the remote attaches as a
normal sibling of the real segment.
- **Before:** a standalone `main <> origin/main` segment with no commits.
- **After:** `main` is a ref on the commit; `origin/main` is the sibling of the
  segment that owns the commit.

## 4. The target does not get naming priority; it is treated as a remote/sibling
At a commit shared by the target's local branch and a workspace stack branch,
the walk named the segment after the **target** (`main`) and floated the stack
branch empty. We drop the special case: naming is uniform — a workspace stack
branch (metadata) names the segment; the target is just a ref + remote sibling.
- Disambiguation of *which local branch names a shared commit's segment* is
  **remote-tracked branch → the only branch → anonymous** (the flip's
  `disambiguated_ref`). This is NOT a simplification — it reproduces the walk's
  remote-local-tracking naming (the walk names a segment after the local branch
  whose remote points at that commit, e.g. `main` for `origin/main`). An earlier
  note here claimed a "metadata → remote → single" rule; that was wrong — there
  is no metadata tier, and switching to one regresses parity.

## 8. `InWorkspace` is message-driven, not metadata-driven (KEEP FLIP)
A `gitbutler/workspace` checkout whose merge commit has the managed-workspace
message marks its reachable commits `InWorkspace` (`🏘`) even when there is **no
workspace metadata**. The walk instead treats a no-metadata workspace checkout as
a normal branch (no `🏘`) — see `minimal_merge`'s "the branch is normal!" comment.
Decision (Mattias): **keep the flip** — the message is the more robust signal in
production where metadata can lag. The walk is NOT retrofitted (reclassifying a
no-metadata checkout as a `TipRole::Workspace` tip would reshape target
resolution + stack materialization in a soon-to-be-deleted codebase); the
affected walk snapshots (`minimal_merge*`, `without_target_ref_*managed_commit*`,
…) are re-accepted to the flip's output when the flip becomes the default.

## 6. Worktrees are KEPT (CLI supports them)
Not a simplification — the flip must reproduce the walk's worktree annotations:
the main worktree `[🌳]`, linked worktrees `[📁]`, and the `@repo` ownership
marker (which worktree the building repo owns). This is a feature the flip needs
to add, not remove.

## Deferred (no decision yet)
- **#5 inactive/outside stacks as empty segments** — kept as-is for now (the
  metadata is retained for PR/branch-info survival; the projection filters them).
- **#7 shared remote-ahead ownership** — when one commit carries two remote refs,
  how they stack. Deferred.

# Execution readiness (deleting the SegmentGraph walk) — DO NOT EXECUTE YET

Everything below is staged; per Mattias, the old code is NOT deleted until he
says so.

## Architecture as it stands
- `graph_from_repository` (managed) runs `CommitGraph::from_walk` — the REAL
  `Graph::from_commit_traversal` with `dangerously_skip_postprocessing_for_debugging`,
  flattened by `CommitGraph::from_segment_graph`. Extents, limits, and flags are
  the walk's by construction; `walk_names` records the traversal's tip-seeded
  segment names (order-dependent, not statically reproducible). Everything the
  walk's `post.rs` (~1833 lines) does is reproduced by the flip's derived passes
  in `commit_graph_to_segment_graph.rs`, verified at the projection level.
- So the walk's TRAVERSAL is kept (it is good); it is `post.rs` + the projection
  wiring around the raw segments that the flip replaces.

## Dead on the managed flip path (kept per instruction, delete when executing)
- `CommitGraph::from_repository_with_limit` / `from_repository` (budgeted BFS
  include-set) — superseded by `from_walk`; only spike/parity-harness tests use it.
- `CommitGraph::remark_not_in_remote` — flags now come from the walk.
- `CommitGraph::mark_integrated` — same.
- The integrated-ws traversal clip at the top of `graph_from_commit_graph`
  (goal-set pruning when the ws position itself is Integrated) — `from_walk`
  already bounds the set; the clip no longer changes anything on that path.

## Sweep instrumentation (remove LAST — task #16)
Kept togglable in `sweep.patch` (scratchpad), applied to the working tree:
- `init/mod.rs` `from_commit_traversal`: `BUT_GRAPH_PARITY`-gated panic block
  (mirrors the production dispatch: ws-ref tip → from_head form, else forwards
  the entrypoint; skips skip-postprocessing graphs).
- `projection/workspace/mod.rs`: `Workspace::projection_fingerprint()`.
Run: `BUT_GRAPH_PARITY=1 cargo test -p but-graph --test graph --no-fail-fast -- --test-threads=1`.
Commit workflow: `git diff <2 files> > sweep.patch && git restore <2 files>`,
fmt+commit, `git apply sweep.patch`.

## Flip-default order (task #14+)
1. Route `Graph::from_head` / `from_commit_traversal` through
   `graph_from_repository` (managed) / `graph_from_repository_unmanaged`
   behind `BUT_GRAPH_FLIP`, then default it on.
2. Re-accept walk snapshots that encode the intentional items above
   (structural diffs only; projections already agree). #8's list:
   `minimal_merge*`, `without_target_ref_*managed_commit*`, ….
3. Run but-workspace / but-rebase / e2e Playwright suites as guardrails (#15).
4. Delete: `post.rs`, the walk-only projection glue, the dead CommitGraph
   methods above, then the sweep instrumentation (#16).
