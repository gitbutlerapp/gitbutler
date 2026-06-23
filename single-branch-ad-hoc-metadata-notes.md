# Single-Branch Ad-Hoc Metadata Notes

## Motivation

Single-branch mode can create multiple local branch refs that all point at the same commit. Without additional metadata, Git alone cannot express which empty branch should appear above or below another empty branch in the UI.

The work on `single-branch-ad-hoc-metadata` adds persisted ad-hoc branch ordering so the app can keep same-tip empty branches in a stable stack order. The follow-up Playwright work covers the user-visible Tauri app flows for creating and removing these empty branches in single-branch mode.

## Relevant Concepts

- `branch_order` database table: stores ad-hoc branch order as a parent chain, from top branch toward the base branch.
- `BranchOrderMetadata`: bridges legacy metadata with DB-backed ad-hoc ordering.
- `Anchor::AtReference`: creates a branch at the same commit as the anchor ref, using the requested position only for ordering.
- Ad-hoc workspace: a single-branch workspace without managed workspace metadata, where the checked-out local branch is treated as the workspace entrypoint.
- Empty branch segment: a graph/workspace segment with a ref but no commits. These must be preserved if they are part of persisted ad-hoc ordering.
- Projection pruning: workspace projection removes integrated commits and normally prunes empty branches; DB-ordered ad-hoc empty branches need to survive that pruning.
- Checked-out branch deletion: Git refuses to delete a checked-out ref, so removing a checked-out empty ad-hoc branch must first move `HEAD` to a surviving neighboring branch.

## Testing

Backend and graph coverage:

- Added/updated Rust coverage for creating a branch below an empty branch between empty branches.
- Added/updated Rust coverage for removing empty branches from the top, middle, and bottom of ad-hoc stacks.
- Added a `but-api` regression test that exercises the legacy `create_reference` path used by the app and verifies `head_info` includes the inserted empty branch.
- Extended that API test to verify deleting a checked-out empty top branch switches `HEAD` to the next branch before deletion.

Tauri app e2e coverage:

- Added Playwright single-branch tests for:
  - Adding a branch below an empty branch in between empty branches.
  - Removing an empty branch from the top of a two-branch stack.
  - Removing an empty branch from the middle of a three-branch stack.
  - Removing an empty branch from the bottom of a two-branch stack.

Validation run:

- `cargo fmt --check`
- `cargo check -p but-graph -p but-workspace -p but-api`
- `cargo clippy -p but-graph -p but-workspace -p but-api --all-targets`
- `cargo build -p but-server -p but`
- targeted `but-workspace` tests for create/remove scenarios
- targeted `but-api` regression test
- targeted Playwright run for the four new single-branch scenarios
- `corepack pnpm exec prettier --check ...`
- `git diff --check`

Known validation caveat:

- `corepack pnpm --filter @gitbutler/e2e exec tsc --noEmit` still fails on existing PostCSS config typing errors in `packages/shared/postcss.config.js` and `packages/ui/postcss.config.js`.

## Risks

- The graph post-processing and workspace projection changes are subtle; they affect how same-tip local refs become empty stack segments in ad-hoc workspaces.
- Preserving DB-ordered empty segments during projection pruning must not accidentally keep unrelated empty branches alive.
- Switching `HEAD` before deleting a checked-out empty branch changes user-visible state. It is intended for ordered ad-hoc empty branches, but reviewers should confirm this behavior is acceptable for all single-branch deletion flows.
- The frontend now uses `atReference` from the branch header context menu. This is correct for same-tip empty ordering, but reviewers should confirm no managed-workspace branch-header flow depended on `atSegment` semantics there.
- Cache invalidation was broadened for create/remove reference flows; this should keep the UI accurate, but may cause extra workspace refetches.

## Open Questions

- Should checked-out ad-hoc branch deletion always move to the branch below, or should the UI/API choose the nearest surviving branch by explicit policy?
- Should `remove_reference` itself own the checked-out-ref replacement behavior, or is keeping it in the legacy API boundary the right layering?
- Should the app expose clearer UI state for deleting the currently checked-out empty branch, since it may switch `HEAD`?
- Are there multi-worktree edge cases where switching `HEAD` in the current worktree is insufficient or surprising?
- Should all branch-header create operations use `atReference`, or should managed workspaces keep a separate `atSegment` path?

## Critique: Flaws and Testing Gaps

The following review covers correctness bugs, robustness/design issues, testing gaps,
and minor cleanups. File:line references point at the current branch state.

### Critical correctness issues

1. **Renaming an ordered ad-hoc branch silently corrupts the order table.**
   The `branch_order` table keys rows by full ref-name *strings*, but no rename path
   updates it. The UI Rename action → `updateBranchName` →
   `gitbutler_branch_actions::stack::update_branch_name`
   (`crates/but-action/src/rename_branch.rs:46`); a grep across `but-workspace`,
   `but-api`, and `but-meta` shows nothing touches `branch_order` outside the trait
   methods themselves. For a chain `[A, B, C]` where `B` is renamed to `B'`:
   - The stale rows `(A→B)`/`(B→C)` persist; `order_for_reference` keeps returning the
     dead name `B`.
   - In `ad_hoc_branch_stack_upgrades`, `try_find_reference(B)` returns `None` and `B`
     is skipped (`crates/but-graph/src/init/post.rs:1519`), so the renamed branch `B'`
     loses its position and reverts to default placement.
   - Subsequent `set_order`/`remove_reference` operate on a chain containing a phantom
     ref.

   This is a real, user-reachable data-integrity bug with no test coverage.

2. **HEAD switch on delete does not touch the worktree — only safe by luck.**
   `checked_out_ad_hoc_replacement` → `update_head_reference`
   (`crates/but-core/src/repo_ext.rs:26`) calls only `repo.edit_reference("HEAD", …)`
   with `deref=false`. It rewrites the symbolic ref and a reflog entry; it does *not*
   update the index or working tree. This works today only because all same-tip ad-hoc
   refs point at the same commit (same tree). `checked_out_ad_hoc_replacement`
   (`crates/but-api/src/legacy/stack.rs:277`) never verifies the replacement is at the
   same commit — it peels the target purely to compose the reflog `num_parents`. If the
   persisted order ever contains a branch at a different commit (divergence is not
   prevented), HEAD moves without a checkout and the worktree is left on the old tree →
   spurious uncommitted diffs. Needs an explicit same-commit guard (or a real checkout)
   and a test.

3. **No atomicity across Git + DB + metadata.**
   `create_reference` creates the Git ref and re-traverses, persists workspace metadata,
   then finally `set_branch_stack_order`
   (`crates/but-workspace/src/branch/create_reference.rs`, tail). If the DB write fails
   (locked/IO), the ref exists in Git but ordering is never persisted → inconsistent
   state on next load. Symmetrically on delete, `legacy/stack.rs` switches HEAD and
   reloads *before* `remove_reference`; a failure after the HEAD switch leaves HEAD moved
   but the branch undeleted. No compensating rollback exists.

4. **Cycle/corruption in the order table hard-fails the whole workspace load.**
   `order_for_reference` returns `Err(rusqlite::Error::InvalidQuery)` on a detected cycle
   (`crates/but-db/src/table/branch_order.rs:77,87`). That error propagates through
   `branch_stack_order` → `ad_hoc_branch_stack_upgrades` → graph construction with `?`,
   so a single corrupt chain breaks workspace projection entirely. Contrast with the
   missing-table case, deliberately downgraded to `Ok(None)`
   (`crates/but-db/src/table/branch_order.rs:128`). Ordering is a presentation nicety; it
   should degrade gracefully, not brick the view.

### Robustness / design issues

5. **`set_order` silently drops chain members not in the list.**
   `DELETE FROM branch_order WHERE branch_ref_name = ?1 OR parent_ref_name = ?1`
   (`crates/but-db/src/table/branch_order.rs:158`) runs per branch. If an existing row
   `(P→A)` exists where `A` is in the new list but `P` is not, `P` is deleted and
   vanishes from ordering. Masked in practice because callers pass the full chain from
   `branch_stack_order(anchor)`, but it is a sharp edge with no guard and no test.

6. **No garbage collection of stale order rows.**
   Externally deleted branches (`git branch -d`) leave their `branch_order` rows forever.
   Nothing reconciles the table against real refs. `VirtualBranchesTomlMetadata` has a
   `garbage_collect`; the branch-order table has no equivalent. Cruft accumulates and
   feeds back into projection (`keep_empty_segment_ids`, `is_ordered_ad_hoc_lower_bound`)
   via dead names.

7. **The recorded order influences projection even when no segments were rebuilt.**
   `ad_hoc_branch_stack_upgrades` pushes `branch_order.clone()` into
   `graph.ad_hoc_branch_stack_orders` at `crates/but-graph/src/init/post.rs:1491` —
   before the `matching_refs.len() < 2` early return at
   `crates/but-graph/src/init/post.rs:1543`. So when a chain exists but no empty segments
   were materialized (e.g. branches diverged), the order still drives
   `keep_empty_segment_ids` and `is_ordered_ad_hoc_lower_bound` in projection
   (`crates/but-graph/src/projection/workspace/init.rs:435,1151`). The "recorded order"
   and "actually restructured" states can disagree.

8. **`keep_empty_segment_ids` flattens *all* orders.**
   It collects ordered refs from `ad_hoc_branch_stack_orders.iter().flatten()`
   (`crates/but-graph/src/projection/workspace/init.rs:1151`) with no scoping to the
   current stack — exactly the risk the notes call out ("must not accidentally keep
   unrelated empty branches alive"). Single-branch likely has one chain so it is latent,
   but the code permits cross-chain leakage.

9. **Success criterion checks the graph, not the projection.**
   In `create_reference`, the ad-hoc path sets `has_new_ref_as_standalone_segment` by
   scanning raw `graph.node_weights()` rather than `find_segment_and_stack_by_refname`.
   A graph node can exist while projection pruning drops it from the workspace stacks —
   so the op can "succeed" while the UI shows nothing. The two pruning fixes are meant to
   prevent that, but the success signal and the display path are now decoupled and can
   drift.

10. **HEAD-switch layering is split and inconsistent.**
    Delete-side HEAD switching lives in `but-api/legacy/stack.rs`; create-side checkout
    lives in `but-api/branch.rs` (`branch_create_with_perm` drops all guards then calls
    `branch_checkout_with_perm`). Neither lives in `but-workspace`. Two API modules
    independently reason about "is this ref checked out," with duplicated `head_name()`
    logic. Also: `branch_create_with_perm` computes
    `WorkspaceState::from_workspace(&new_ws, …)` unconditionally and discards it whenever
    `checkout_after_create` is true (wasted work), and the auto-checkout's oplog/undo
    granularity (create + checkout as one snapshot or two?) is unspecified and untested.

### Testing gaps

Existing coverage is decent for happy paths (create-below-between-empties; remove
top/middle/bottom; one API regression). Missing:

- **Rename of an ordered empty branch** (issue 1) — untested and broken.
- **Persistence across a cold reload** — all e2e runs are one session. No test that the
  order is re-derived after a fresh `Graph::from_head` / app restart.
- **Auto-checkout-on-create-above** is only unit-tested
  (`crates/but-api/src/branch.rs:1390`), never e2e; and never with a *dirty worktree*
  (does `branch_checkout` carry/refuse WIP changes?).
- **Deleting the checked-out *middle* branch** — the API test only covers deleting the
  checked-out *top*. The replacement-prefers-below logic for a middle deletion is
  unverified.
- **Replacement at a different commit** (issue 2) — no test; this is where the
  worktree-safety bug bites.
- **Cycle / corrupt order row** (issue 4) — no test that load degrades gracefully.
- **Diverged branches in a chain** (`matching_refs < 2` path) — untested.
- **Multiple independent chains** in one project — untested; relevant to issue 8.
- **Reordering** an existing chain (`set_order` rewriting) and the orphan-drop edge
  (issue 5) — untested.
- **DB-write-failure / partial failure** (issue 3) — untested.
- The notes' own caveat: `tsc --noEmit` for `@gitbutler/e2e` still fails (pre-existing
  PostCSS typing). Pre-existing, but it means the new TS tests are not type-checked by
  that gate.

### Minor / cleanliness

- `AddDependentBranchModal` keeps `stackId` in its props type and `BranchList` still
  passes it, but the modal no longer uses it — dead prop. It also hardcodes
  `side: "above"`, so the modal can only ever create above.
- `branchReference` (TextDecoder over `fullNameBytes`) is duplicated in
  `BranchList.svelte` and `BranchHeaderContextMenu.svelte` — candidate for a shared
  helper.
- Broadened invalidation (`invalidatesList(StackDetails)` etc. in `stackEndpoints.ts`)
  trades correctness for extra refetches; acceptable but already flagged.
- `order_for_reference` issues 2N point queries per chain (fine at current scale).

### Highest-priority fixes

1. Keep `branch_order` in sync on rename (and ideally on external deletion via GC) —
   issue 1.
2. Guard the HEAD-switch replacement to same-commit, or perform a real checkout —
   issue 2.
3. Downgrade the cycle error to graceful `Ok(None)` like the missing-table case —
   issue 4.
