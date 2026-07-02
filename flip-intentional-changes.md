# Intentional graph-behavior changes (CommitGraph flip)

> **STATUS 2026-07-02 — THE FLIP IS THE DEFAULT** (commit `ac975ef523`).
> `from_head`, `from_commit_traversal`, and `redo_traversal_with_overlay` build
> managed workspaces via `graph_from_repository(_with_overlay)`;
> `BUT_GRAPH_NO_FLIP` forces the legacy walk until it is deleted. Non-managed
> checkouts and the explicit-tips API (`from_commit_traversal_tips`) remain
> WALK-backed — the latter needs a flip counterpart before deletion. The full
> cargo workspace passes with zero failures and the Playwright e2e suite is
> green, both without any env var. All snapshots are re-accepted to the flip's
> segment numbering (one file pending: but-workspace's `apply_unapply.rs`
> re-accepts are blocked by hunk-locking against another applied stack).
> Projection parity was previously proven exhaustively by the `BUT_GRAPH_PARITY`
> sweep (0 divergences over the whole but-graph suite). The items below describe
> *structural* differences that remain by design.
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
1. ✅ DONE — the dispatch lives in `from_commit_traversal` and
   `redo_traversal_with_overlay` behind `BUT_GRAPH_FLIP`: managed workspaces via
   `graph_from_repository(_with_overlay)`, walk fallback on `Ok(None)`. The
   NON-managed builder is deliberately not routed to (not parity-proven).
   Overlays are served from memory by the flip (`OverlayMetadata: RefMetadata`).
2. ✅ VALIDATED — flip-on failures in but-workspace + but-rebase are pure
   segment-INDEX renumbering (indices differ per-build by construction), except
   TWO but-rebase `workspace_commit_behaviour` fixture guards (`assert_eq`, not
   insta): the flip splits a no-metadata managed ws commit's parents into their
   own segments where the walk keeps them inline — intentional-change #8
   family; hand-edit at flip-default.
3. ✅ e2e Playwright green flip-on (94 passed), repeatedly.
4. ✅ EXECUTED — the flip is the default (`BUT_GRAPH_NO_FLIP` = legacy escape
   hatch); all snapshots re-accepted; full workspace + e2e green without env.
5. BEFORE DELETION: `from_commit_traversal_tips` (explicit tips) is still
   walk-backed; its only non-test production caller is `but-debug`'s revision
   command — give it a flip counterpart or drop it with the walk. Commit the
   blocked `apply_unapply.rs` re-accepts; remove `BUT_GRAPH_NO_FLIP` + the
   sweep instrumentation (#16).
6. Delete: `post.rs`, the walk-only projection glue, the dead CommitGraph
   methods above.

## Debugging aid
`BUT_GRAPH_FLIP_DEBUG=1` prints, on flip entry, the resolved workspace commit,
entrypoint(+ref), and the FULL overlay. This is how apply/unapply preview
overlays were captured and replayed to fix every operation-flow divergence —
keep until deletion is complete.
