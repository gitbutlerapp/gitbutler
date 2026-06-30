# Single-Branch Ad-Hoc Metadata PR Plan

## Goal

Land single-branch ad-hoc branch ordering incrementally, without introducing a large backend rewrite or a broad UI/API migration in one review.

The core invariant for the series:

> Ad-hoc branch-order metadata may improve display and branch operations, but invalid, stale, or externally changed metadata must never prevent the workspace from loading or corrupt Git state.

## Motivation

Single-branch mode can create multiple local branch refs at the same commit. Git alone cannot express a stable visual order for those empty same-tip branches, so GitButler needs additional metadata.

The current branch adds DB-backed `branch_order` metadata and uses it during graph/projection to keep empty branches ordered and visible. That solves the immediate UI problem, but the critique identified several flaws:

- stale metadata after rename or external deletion
- corrupt/cyclic metadata breaking workspace load
- unsafe `HEAD` switching assumptions during checked-out branch deletion
- projection behavior that can preserve too much or rely on metadata that was not actually materialized
- split checkout/delete logic across legacy and newer API surfaces
- insufficient API and e2e coverage for reloads, rename, divergence, corruption, and checked-out deletion variants

The plan below breaks the work into PR-sized phases with narrow review surfaces.

## Phase 0: Baseline And Scope Lock

### Purpose

Make the current branch easy to review before changing behavior further.

### Scope

- Keep existing implementation changes unamended until inspected.
- Capture the current failing/passing validation state.
- Confirm the minimum user-visible flows this feature must support:
  - create empty branch above/below another empty branch
  - remove empty branch from top/middle/bottom of an ordered stack
  - reload and preserve order
  - rename ordered branch without losing order
  - tolerate external branch deletion

### Tests / Validation

- Re-run the already targeted Rust tests.
- Re-run the targeted Playwright single-branch tests.
- Record known unrelated `@gitbutler/e2e` TypeScript caveat if still present.

### Non-Goals

- No new APIs.
- No frontend migration.
- No metadata schema change unless required by later phases.

### Review Boundary

This can be a documentation-only PR or an internal checkpoint before implementation PRs.

## Phase 1: Harden Metadata Loading And Failure Behavior

### Purpose

Make existing `branch_order` metadata safe to read. This should land before expanding API usage.

### Scope

- Treat cyclic/corrupt branch-order chains as invalid ordering, not as fatal workspace-load errors.
- Tolerate missing refs in an order chain.
- Ensure stale rows do not cause graph/projection construction to fail.
- Add logging/tracing where helpful so corruption can be diagnosed without breaking the UI.

### Candidate Code Areas

- `crates/but-db/src/table/branch_order.rs`
- `crates/but-graph/src/init/post.rs`
- `crates/but-graph/src/projection/workspace/init.rs`

### Tests

- `but-db` or `but-workspace` test: cycle in `branch_order` degrades to no order.
- `but-workspace` test: ordered ref missing from Git does not fail workspace load.
- `but-workspace` test: externally deleted middle branch produces a stable remaining projection.
- Optional snapshot/assertion: no phantom branch is shown when only metadata remains.

### Validation

- `cargo test -p but-db`
- targeted `cargo test -p but-workspace`
- `cargo check -p but-db -p but-graph -p but-workspace`
- `cargo clippy -p but-db -p but-graph -p but-workspace --all-targets`
- `cargo fmt --check`

### Non-Goals

- Do not add rename support yet.
- Do not introduce new API endpoints.
- Do not change frontend behavior.

### Review Boundary

Reviewers should only need to answer: "Can invalid metadata still break workspace load?"

## Phase 2: Reconcile Metadata With Git Refs

### Purpose

Define and implement what happens when Git refs change outside the metadata writer.

### Scope

- Add a reconciliation or garbage-collection path for `branch_order`.
- Remove or ignore rows for refs that no longer exist.
- Decide whether reconciliation happens:
  - opportunistically during metadata read,
  - explicitly during workspace load,
  - or through a metadata maintenance call.
- Ensure reconciliation is conservative and does not delete valid order when refs are temporarily unavailable.

### Candidate Code Areas

- `crates/but-db/src/table/branch_order.rs`
- metadata abstraction around `BranchOrderMetadata`
- workspace/graph initialization code that already has access to real refs

### Tests

- External deletion of top/middle/bottom ordered branch.
- Cold reload after external deletion.
- Stale lower-bound row does not keep unrelated empty branches alive.
- Multiple independent stale chains do not affect the active stack.

### Validation

- targeted `but-db` tests
- targeted `but-workspace` tests
- `cargo check -p but-db -p but-workspace -p but-graph`
- `cargo clippy -p but-db -p but-workspace -p but-graph --all-targets`
- `cargo fmt --check`

### Non-Goals

- No UI changes.
- No rename API yet, except possibly low-level helper tests that make rename support possible.

### Review Boundary

Reviewers should only need to answer: "Does metadata stay harmless and eventually clean when Git refs disappear?"

## Phase 3: Rename-Safe Branch Order

### Purpose

Fix the known data-integrity bug where renaming an ordered ad-hoc branch leaves stale branch-order rows behind.

### Scope

- Add a metadata operation for ref rename, for example `rename_reference(old, new)`.
- Update all rows where either `branch_ref_name` or `parent_ref_name` matches the old full ref name.
- Ensure rename preserves order for top, middle, bottom, and checked-out branches.
- Wire the existing rename path to update branch-order metadata.

### Candidate Code Areas

- `crates/but-db/src/table/branch_order.rs`
- `crates/but-action/src/rename_branch.rs`
- related metadata trait/adapter code

### Tests

- Rename top branch in ordered stack.
- Rename middle branch in ordered stack.
- Rename bottom branch in ordered stack.
- Cold reload after rename preserves order.
- Rename of an unordered branch does not create metadata.
- Rename collision/error path does not partially rewrite metadata.

### Validation

- targeted `but-action` or integration tests around rename
- targeted `but-workspace` reload/projection tests
- `cargo check -p but-action -p but-workspace -p but-db`
- `cargo clippy -p but-action -p but-workspace -p but-db --all-targets`
- `cargo fmt --check`

### Non-Goals

- Do not introduce the new non-legacy rename API in this phase unless the existing path cannot be fixed cleanly.
- Do not migrate frontend rename calls.

### Review Boundary

Reviewers should only need to answer: "Does branch-order metadata remain correct after existing rename operations?"

## Phase 4: Projection Semantics And Stack Scoping

### Purpose

Make graph/projection behavior consistently informed by valid branch-order metadata, without cross-chain leakage.

### Scope

- Scope preserved empty segments to the active ad-hoc stack/order, not every known order.
- Only let metadata affect projection when the ordered chain is valid enough to materialize.
- Ensure success criteria for branch creation/removal match projected workspace state, not only raw graph node presence.
- Clarify diverged-chain behavior:
  - same-tip refs may be ordered as empty branches
  - diverged refs should not be forced into the same empty stack
  - invalid order should degrade to normal placement

### Candidate Code Areas

- `crates/but-graph/src/init/post.rs`
- `crates/but-graph/src/projection/workspace/init.rs`
- `crates/but-workspace/src/branch/create_reference.rs`
- branch remove/reference tests under `crates/but-workspace/tests`

### Tests

- Same-tip ordered empty stack projects in persisted order.
- Diverged branch in the chain does not get forced into the empty stack.
- Multiple independent chains do not keep each other's empty segments alive.
- Create-reference success requires the new ref to appear in projected workspace output.
- Cold reload preserves the same projected order.

### Validation

- targeted `cargo test -p but-graph`
- targeted `cargo test -p but-workspace`
- `cargo check -p but-graph -p but-workspace`
- `cargo clippy -p but-graph -p but-workspace --all-targets`
- `cargo fmt --check`

### Non-Goals

- No frontend changes.
- No API migration.
- No new storage format unless existing metadata cannot support correct scoping.

### Review Boundary

Reviewers should only need to answer: "Does projection preserve exactly the ordered empty branches it should, and no more?"

## Phase 5: Shared Checkout And Checked-Out Deletion Primitive

### Purpose

Centralize the logic for changing the checked-out branch during create/remove operations.

### Scope

- Introduce one shared backend helper for safe checked-out ref replacement.
- Require same-commit replacement for symbolic `HEAD` updates.
- Use a real checkout, or reject the operation, when replacement points at a different commit.
- Define replacement policy for deletion:
  - top deletion prefers next branch below
  - middle deletion prefers below, unless policy chooses nearest surviving branch
  - bottom deletion prefers branch above
- Define oplog/undo expectations for create+checkout and remove+checkout.

### Candidate Code Areas

- `crates/but-api/src/legacy/stack.rs`
- `crates/but-api/src/branch.rs`
- possible shared helper in `but-workspace` or a lower-level crate
- `crates/but-core/src/repo_ext.rs`

### Tests

- Delete checked-out top branch.
- Delete checked-out middle branch.
- Delete checked-out bottom branch.
- Replacement at same commit updates `HEAD` without worktree diff.
- Replacement at different commit is rejected or performs a real checkout, depending on chosen policy.
- Dirty worktree behavior is explicit and tested.

### Validation

- targeted `cargo test -p but-api`
- targeted `cargo test -p but-workspace`
- `cargo check -p but-api -p but-workspace -p but-core`
- `cargo clippy -p but-api -p but-workspace -p but-core --all-targets`
- `cargo fmt --check`

### Non-Goals

- Do not add new public branch APIs in this phase.
- Do not migrate frontend calls yet.

### Review Boundary

Reviewers should only need to answer: "Is checked-out branch movement safe, consistent, and tested?"

## Phase 6: Non-Legacy Rename API

### Purpose

Add the first modern API that is explicitly compatible with single-branch ad-hoc metadata.

### Scope

- Add or update a non-legacy rename API.
- Use the rename-safe metadata operation from Phase 3.
- Use shared checkout behavior if renaming the checked-out branch needs it.
- Return enough workspace/head state for the frontend to update consistently.

### Tests

- API rename preserves ordered stack.
- API rename checked-out branch preserves `HEAD`.
- API rename followed by cold reload preserves order.
- API rename fails cleanly on collision without partial metadata changes.

### Validation

- targeted `cargo test -p but-api`
- `cargo check -p but-api`
- `cargo clippy -p but-api --all-targets`
- `cargo fmt --check`

### Non-Goals

- Do not migrate every frontend rename call unless the API is ready and the diff stays small.
- Do not add remove/create API changes here.

### Review Boundary

Reviewers should only need to answer: "Is the new rename API correct for ordered single-branch stacks?"

## Phase 7: Non-Legacy Remove API

### Purpose

Add a modern remove API that safely handles ordered ad-hoc branches and checked-out deletion.

### Scope

- Add or update a non-legacy remove/delete branch API.
- Use the shared checked-out deletion primitive from Phase 5.
- Remove or reconcile branch-order metadata atomically enough for the current storage design.
- Return projected workspace/head state after deletion.

### Tests

- Remove top/middle/bottom ordered empty branch through API.
- Remove checked-out top/middle/bottom ordered empty branch through API.
- Remove unordered branch does not mutate unrelated order.
- Failure after `HEAD` replacement or metadata update is handled or documented.

### Validation

- targeted `cargo test -p but-api`
- targeted `cargo test -p but-workspace`
- `cargo check -p but-api -p but-workspace`
- `cargo clippy -p but-api -p but-workspace --all-targets`
- `cargo fmt --check`

### Non-Goals

- No frontend migration unless kept to a separate very small PR.
- No remote branch handling.

### Review Boundary

Reviewers should only need to answer: "Does the new remove API update Git, metadata, HEAD, and projected state consistently?"

## Phase 8: Non-Legacy Create / Move / Remote APIs

### Purpose

Expand modern API coverage after metadata, projection, rename, remove, and checkout semantics are already hardened.

### Scope

- Add or update create branch API for single-branch ad-hoc ordering.
- Add move/reorder support if it is user-facing and needed for the feature.
- Add remote-related API behavior only after local ordering semantics are stable.
- Ensure all APIs share the same metadata writer and checkout primitive.

### Tests

- Create above/below same-tip empty branch.
- Create below an empty branch between empty branches.
- Create with checkout-after-create.
- Move/reorder existing ordered branches, if supported.
- Remote branch behavior, if added:
  - applying remote branch does not corrupt local ad-hoc order
  - deleting/renaming local branch with upstream metadata is safe

### Validation

- targeted `cargo test -p but-api`
- targeted `cargo test -p but-workspace`
- `cargo check -p but-api -p but-workspace -p but-graph`
- `cargo clippy -p but-api -p but-workspace -p but-graph --all-targets`
- `cargo fmt --check`

### Non-Goals

- Do not combine all create/move/remote work if each can stand alone.
- Prefer one operation per PR if the implementation is not obviously mechanical.

### Review Boundary

Reviewers should only need to answer: "Does this specific new API operation preserve the established invariants?"

## Phase 9: Frontend Migration And Cache Invalidation

### Purpose

Move the desktop app to the hardened API surface with minimal behavioral changes.

### Scope

- Replace legacy calls one flow at a time.
- Keep branch-header create semantics explicit:
  - `Anchor::AtReference` for same-tip empty branch ordering
  - preserve managed-workspace behavior if it still requires `AtSegment`
- Tighten cache invalidation to the minimum needed after each operation.
- Remove dead props/helpers exposed by the earlier branch-header changes.

### Candidate Code Areas

- `apps/desktop/src/components/branch/BranchHeaderContextMenu.svelte`
- `apps/desktop/src/lib/stacks/stackEndpoints.ts`
- any branch rename/remove/create frontend API wrappers

### Tests

- Unit or component tests if existing patterns support them.
- Prettier check for touched Svelte/TS files.
- TypeScript check where possible, documenting unrelated known failures if still present.

### Validation

- `corepack pnpm exec prettier --check <touched files>`
- targeted package typecheck if available
- targeted Playwright smoke tests for migrated flows

### Non-Goals

- Do not add broad e2e coverage in the same PR if API migration is already non-trivial.
- Do not redesign branch UI.

### Review Boundary

Reviewers should only need to answer: "Did the frontend switch to the new safe API without changing unrelated UI behavior?"

## Phase 10: Playwright E2E Coverage

### Purpose

Verify the user-visible Tauri app behavior after backend invariants and frontend calls are stable.

### Scope

- Add Playwright tests for primary single-branch empty-stack flows.
- Keep tests focused on user-visible outcomes:
  - branch appears in the expected order
  - branch is removed
  - checked-out branch changes when expected
  - order survives reload/reopen

### Tests

- Add branch below an empty branch between empty branches.
- Remove empty branch from top of two-branch stack.
- Remove empty branch from middle of three-branch stack.
- Remove empty branch from bottom of two-branch stack.
- Rename ordered empty branch.
- Delete checked-out top/middle/bottom empty branch.
- Reload/reopen app preserves ordered stack.
- Optional dirty-worktree scenario if the UI permits the operation.

### Validation

- targeted Playwright run for the new single-branch tests
- rebuild `but-server` before Playwright when backend changed
- `corepack pnpm exec prettier --check <touched e2e files>`
- document existing `tsc --noEmit` caveat if unchanged

### Non-Goals

- Do not use Playwright to prove low-level corruption behavior.
- Do not combine with backend correctness changes.

### Review Boundary

Reviewers should only need to answer: "Does the app behave correctly for the supported single-branch workflows?"

## Suggested PR Grouping

If we want the fewest PRs while staying reviewable:

1. Metadata hardening and reconciliation: Phases 1-2.
2. Rename-safe metadata: Phase 3.
3. Projection correctness: Phase 4.
4. Shared checkout/delete primitive: Phase 5.
5. New APIs, split by operation: Phases 6-8, likely 2-3 PRs.
6. Frontend migration: Phase 9.
7. Playwright coverage: Phase 10.

If we want maximum review clarity:

1. Phase 1 only.
2. Phase 2 only.
3. Phase 3 only.
4. Phase 4 only.
5. Phase 5 only.
6. Phase 6 only.
7. Phase 7 only.
8. Phase 8 split into create, move, and remote PRs.
9. Phase 9 only.
10. Phase 10 only.

## Cross-Phase Design Decisions

These should be decided before or during the first implementation PR that needs them:

- Should invalid branch-order metadata be deleted immediately, or ignored until explicit GC?
- Should checked-out deletion prefer the branch below, above, or nearest surviving ordered branch?
- Should different-commit replacement perform a real checkout or reject the operation?
- Should metadata writes be best-effort, transactional with DB only, or compensated when Git operations succeed and DB operations fail?
- Should `branch_order` store full ref names permanently, or eventually use a more resilient identifier?
- Should managed workspaces continue using `Anchor::AtSegment` while ad-hoc branch-header flows use `Anchor::AtReference`?

## General Validation Gate Per Backend PR

Use the narrowest package set that covers touched code, but the expected baseline is:

- `cargo fmt --check`
- targeted `cargo test`
- targeted `cargo check`
- targeted `cargo clippy --all-targets`

For PRs touching the Tauri app or e2e tests:

- rebuild `but-server` when backend behavior changed
- targeted Playwright run
- Prettier check for touched Svelte/TypeScript files
- TypeScript check when the package gate is healthy, otherwise document known unrelated failures

## Landing Strategy

Prefer landing the safety phases before feature expansion:

1. Make metadata harmless.
2. Make metadata consistent.
3. Make projection precise.
4. Make checkout/delete shared and safe.
5. Add modern APIs.
6. Migrate UI.
7. Prove the full user journey with e2e tests.

This keeps each review centered on one question and avoids turning a UI ordering feature into a single high-risk backend migration.
