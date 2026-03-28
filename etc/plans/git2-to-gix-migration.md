# git2 to gix Migration Plan (Single File, Scoped)

## Summary

This plan orchestrates migration from `git2` to `gix` across the codebase where possible, while treating only these areas as hard boundaries:

- checkout execution / worktree materialization
- index/tree materialization that still requires `git2` (`Index::write_tree*`, `read_tree`, and immediately related adapters)
- `gitbutler-edit-mode` as a legacy checkout/edit-flow boundary crate until its remaining `git2` checkout/index handoff is isolated or replaced

`create_wd_tree()` itself is already implemented in `gix` in `but-core`; remaining `git2` usage around it should be treated as legacy wrapper/caller cleanup, not as a true `git2` implementation boundary.

Config reading and config-setting are explicitly in-scope for migration in this plan. They should use the existing `git_config.rs` / `gix`-based config helpers and must not be treated as a valid reason to keep a `git2` boundary.

Transport strategy remains dual-backend for now, but direct `git2` transport/auth call-sites are in-scope for cleanup where `gix` or existing path-based helpers are sufficient.

Tests and fixture helpers are also explicitly in-scope. This is not just cleanup: repository isolation is part of the migration goal, and `gix` can be opened with isolated options in a way that `git2` does not expose as an equivalent configurable mode for our test setup.

This document was reconciled against the repository on 2026-03-21 using `rg` and `cargo metadata`.

## Baseline and Scope

Repository scan baseline at plan creation:

- `git2::` callsites: 566
- files with `git2::` references: 88
- files currently in-scope under exclusions: 35

Current raw audit on 2026-03-21:

- `git2::` callsites: 199
- files with `git2::` references: 25
- `ctx.git2_repo` / `ctx.with_git2_repo` dot-access matches: 26
- all `git2_repo` / `with_git2_repo` identifier matches including field/setup definitions: 76 across 17 files
- crate manifests with an active `git2` dependency declaration: 3
- `cargo fmt --check --all`: passing
- `cargo clippy --all-targets --workspace`: passing
- filtered in-scope audit: residual `git2` usage is now concentrated in checkout/materialization boundaries, hook and transport/auth adapters, optional legacy compatibility crates, and deliberate boundary-coverage tests

In-scope means all non-excluded `git2` usage should be migrated to `gix` or isolated behind explicit adapter boundaries.

Out-of-scope means only the exact hard boundary remains intentionally on `git2`:

- checkout execution and worktree materialization
- index/tree materialization from staged state (`write_tree/read_tree`, related)

## Migration Goals

1. Eliminate non-excluded direct `git2` usage from application/business logic.
2. Migrate public/internal data models from `git2` IDs/time to `gix` IDs/time where not blocked by exclusions.
3. Preserve JSON/TOML wire formats and behavior.
4. Reduce crate-level `git2` dependencies where no longer needed.
5. Primary success signal: remove `git2` as a non-dev dependency from every crate that does not need it for an explicitly excluded scope or a temporary compatibility boundary.
6. Secondary enforcement signal: once remaining `git2` test/runtime uses are narrowed to accepted boundaries, mark `ctx.git2_repo` as deprecated so new call-sites are explicitly discouraged.

## Dependency Cleanup Priority

High-value manifest targets for this effort are all `but-*` crates.

Current `but-*` non-dev `git2` dependency audit on 2026-03-21:

- `crates/but`: no direct normal `git2` dependency
- `crates/but-core`: still needed for the checkout/worktree hard boundary in `but-core::worktree::checkout::*`
- `crates/but-ctx`: still needed because `Context` owns the deprecated `git2_repo` boundary cache and compatibility callers still rely on that path
- `crates/but-oxidize`: intentionally depends on `git2` as the conversion boundary crate
- `crates/but-serde`: still isolated behind the optional `legacy` feature
- `crates/but-workspace`: still has an optional `git2` dependency and legacy workspace flows still route through `git2` helpers

Implication:

- The highest-value cleanup work is to avoid introducing new non-dev `git2` dependencies in `but-*` crates.
- For `but-*` crates that still depend on `git2`, the preferred end state is either removal, or feature/boundary isolation with a clear excluded-scope justification.
- That end state is not yet reached under the narrowed boundary: `but-core` still owns the remaining hard boundary, while `but-workspace`, `but-ctx`, `but-napi`, and adjacent legacy crates still have additional actionable cleanup.

## Public API/Type Direction

Prefer these replacements in non-excluded areas:

- `git2::Oid` -> `gix::ObjectId`
- `git2::Time` -> `gix::date::Time`
- `git2::Signature` use-sites -> `gix::actor::Signature` / `SignatureRef` at domain boundaries

Compatibility requirements:

- Keep existing serialized hex OID string representation.
- Keep existing key names and payload shape for API/frontend consumers.
- Keep persisted data backward-compatible (especially oplog metadata).

## Workstreams

### Workstream A: Foundational Type and Serde Alignment

Target modules:

- `crates/but-schemars/src/lib.rs`
- `crates/but-serde/src/lib.rs`
- `crates/but-ctx/src/lib.rs`
- `crates/but-workspace/src/ui/author.rs`
- `crates/gitbutler-project/src/lib.rs`
- `crates/gitbutler-repo/src/lib.rs`

Tasks:

1. Add/expand `gix` serde helpers and prefer them in migrated surfaces.
2. Reduce `git2` in shared utility type conversions not tied to excluded domains.
3. Keep explicit `git2`<->`gix` conversion only at necessary boundaries.

Acceptance:

1. Serialization roundtrips remain unchanged for IDs/time fields.
2. No new direct `git2` imports introduced in modern/non-excluded paths.

Current reconciliation notes:

- `crates/but-workspace/src/ui/author.rs` and `crates/gitbutler-project/src/project.rs` match the intended `gix`-first direction.
- `crates/but-serde/src/lib.rs` and `crates/but-schemars/src/lib.rs` still retain legacy `git2` helpers for compatibility; there is no further actionable migration work here without deleting public compatibility surfaces.
- `crates/but-ctx/src/lib.rs` still exposes `git2_repo`, but it is now explicitly deprecated and documented as a boundary-only escape hatch.
- `crates/gitbutler-project/src/lib.rs` is no longer a live `git2` migration target in the current tree.
- `crates/gitbutler-repo/src/lib.rs` still exposes legacy `git2` signature helpers as a compatibility boundary.

### Workstream B: Config and Refname Migration

Target modules:

- `crates/but/src/command/alias.rs`
- `crates/but/src/command/config.rs`
- `crates/but-api/src/legacy/git.rs`
- `crates/gitbutler-repo/src/config.rs`
- `crates/gitbutler-reference/src/refname/mod.rs`
- `crates/gitbutler-reference/src/refname/local.rs`
- `crates/gitbutler-reference/src/refname/remote.rs`
- `crates/gitbutler-reference/src/refname/error.rs`

Tasks:

1. Replace `git2::Config` usage with `gix` config equivalents where practical.
2. Remove direct `TryFrom<&git2::Branch>` dependency from refname parsing path; use refname strings and `gix` refs where possible.
3. Preserve existing not-found and scope semantics in CLI behavior.

Acceptance:

1. Alias/config commands behave identically from user perspective.
2. Refname parse/format behavior remains backward-compatible.

Current reconciliation notes:

- `crates/but/src/command/alias.rs`, `crates/but/src/command/config.rs`, `crates/but-api/src/legacy/git.rs`, `crates/gitbutler-repo/src/config.rs`, and the `gitbutler-reference` refname modules no longer have direct `git2` usage.
- Wave 2 remains complete; no actionable migration tasks remain here.

### Workstream C: Legacy Domain OID/Commit Surface Cleanup (Non-Excluded)

Target modules:

- `crates/but/src/command/commit/move.rs`
- `crates/but/src/command/legacy/branch/list.rs`
- `crates/but/src/command/legacy/branch/show.rs`
- `crates/but/src/command/legacy/pick.rs`
- `crates/but/src/command/legacy/rub/squash.rs`
- `crates/but/src/command/legacy/show.rs`
- `crates/but-api/src/legacy/virtual_branches.rs`
- `crates/gitbutler-branch-actions/src/actions.rs`
- `crates/gitbutler-branch-actions/src/author.rs`
- `crates/gitbutler-branch-actions/src/base.rs`
- `crates/gitbutler-branch-actions/src/branch.rs`
- `crates/gitbutler-branch-actions/src/hooks.rs`
- `crates/gitbutler-branch-actions/src/integration.rs`
- `crates/gitbutler-branch-actions/src/move_commits.rs`
- `crates/gitbutler-branch-actions/src/remote.rs`
- `crates/gitbutler-branch-actions/src/reorder.rs`
- `crates/gitbutler-branch-actions/src/squash.rs`
- `crates/gitbutler-branch-actions/src/stack.rs`
- `crates/gitbutler-branch-actions/src/undo_commit.rs`
- `crates/gitbutler-branch-actions/src/upstream_integration.rs`
- `crates/gitbutler-branch-actions/src/virtual.rs`
- `crates/gitbutler-cherry-pick/src/lib.rs`
- `crates/gitbutler-cherry-pick/src/repository_ext.rs`
- `crates/gitbutler-commit/src/commit_ext.rs`
- `crates/but-workspace/src/legacy/head.rs`
- `crates/but-workspace/src/legacy/integrated.rs`
- `crates/but-workspace/src/legacy/stacks.rs`
- `crates/gitbutler-edit-mode/src/lib.rs`
- `crates/gitbutler-edit-mode/src/commands.rs`
- `crates/gitbutler-stack/src/stack.rs`
- `crates/gitbutler-stack/src/stack_branch.rs`
- `crates/gitbutler-stack/src/state.rs`
- `crates/gitbutler-stack/src/target.rs`

Tasks:

1. Migrate function signatures and local structs from `git2` IDs to `gix` IDs where not crossing excluded paths.
2. Replace direct `git2` commit/tree access in non-excluded logic with `gix` equivalents.
3. For mixed files, isolate excluded operations behind narrow adapters and migrate surrounding logic.

Acceptance:

1. Non-excluded branch/cherry-pick/legacy-integrated flows compile with `gix`-first types.
2. Remaining `git2` in these modules is only for excluded-boundary interaction.

Current reconciliation notes:

- The files in this workstream that still have direct `git2::` usage today are `crates/gitbutler-branch-actions/src/integration.rs`, `crates/gitbutler-edit-mode/src/lib.rs`, `crates/gitbutler-stack/src/stack.rs`, `crates/gitbutler-workspace/src/branch_trees.rs`, `crates/gitbutler-cherry-pick/src/lib.rs`, `crates/gitbutler-cherry-pick/src/repository_ext.rs`, and `crates/gitbutler-commit/src/commit_ext.rs`.
- Additional branch/workspace flows that still depend on the legacy context boundary via `ctx.git2_repo` are now limited to explicit checkout/materialization callers: `crates/gitbutler-branch-actions/src/base.rs`, `crates/gitbutler-branch-actions/src/branch_manager/branch_removal.rs`, `crates/gitbutler-branch-actions/src/integration.rs`, `crates/gitbutler-edit-mode/src/lib.rs`, `crates/gitbutler-oplog/src/oplog.rs`, and `crates/gitbutler-workspace/src/branch_trees.rs`.
- `crates/gitbutler-branch-actions/src/author.rs`, `crates/gitbutler-branch-actions/src/hooks.rs`, `crates/gitbutler-branch-actions/src/remote.rs`, `crates/gitbutler-branch-actions/src/reorder.rs`, `crates/gitbutler-branch-actions/src/undo_commit.rs`, `crates/gitbutler-branch-actions/src/upstream_integration.rs`, and `crates/gitbutler-branch-actions/src/branch_upstream_integration.rs` no longer have direct `git2::` usage.
- Workstream C is complete within the current boundary: the remaining call-sites in these modules are the accepted checkout/index bridge.

### Workstream D: Oplog Metadata and State Modernization (Non-Excluded)

Target modules:

- `crates/gitbutler-oplog/src/entry.rs`
- `crates/gitbutler-oplog/src/oplog.rs`
- `crates/gitbutler-oplog/src/snapshot.rs`
- `crates/gitbutler-oplog/src/state.rs`
- `crates/gitbutler-oplog/tests/oplog/main.rs`

Tasks:

1. Migrate snapshot metadata/state structures from `git2` OID/time types to `gix` equivalents.
2. Preserve on-disk/read compatibility in `operations-log.toml` and snapshot message parsing.
3. Keep only the exact checkout/materialization and tree-creation boundary unchanged and boundary-adapted.

Acceptance:

1. Existing snapshot metadata loads and writes without schema regression.
2. Oplog tests pass with migrated non-excluded types.

Current reconciliation notes:

- Snapshot metadata/state surfaces in `entry.rs`, `snapshot.rs`, `state.rs`, and `tests/oplog/main.rs` are migrated to `gix`.
- `crates/gitbutler-oplog/src/oplog.rs` still uses `git2` in restore/diff/prepare helpers that cross the remaining hard boundary, and remains the main production target here.
- `crates/gitbutler-oplog/src/reflog.rs` test scaffolding is now `gix`-based.
- Workstream D remains active for `oplog.rs` boundary extraction; `reflog.rs` is no longer part of the residual test cleanup list.

### Workstream G: Repo Adapter, Transport, and Legacy Boundary Reduction

Target modules:

- `crates/gitbutler-repo/src/repository_ext.rs`
- `crates/gitbutler-repo/src/commands.rs`
- `crates/gitbutler-repo/src/rebase.rs`
- `crates/gitbutler-repo/src/credentials.rs`
- `crates/gitbutler-repo/src/hooks.rs`
- `crates/gitbutler-repo/src/managed_hooks.rs`
- `crates/gitbutler-repo/src/remote.rs`
- `crates/gitbutler-repo/src/staging.rs`
- `crates/gitbutler-repo-actions/src/repository.rs`
- `crates/but-napi/src/lib.rs`
- `crates/gitbutler-tauri/src/projects.rs`
- `crates/gitbutler-stack/src/stack.rs`

Tasks:

1. Replace direct `git2` repo/logging/remote helpers with `gix`-first equivalents where no hard boundary is involved.
2. Reduce transport/auth `git2` usage to explicit backend adapters instead of leaking `git2` types through business logic.
3. Shrink `ctx.git2_repo` callers to the exact remaining hard boundary and compatibility/public API surfaces.

Acceptance:

1. Read-side repo helpers, ref/remote inspection, and commit traversal no longer require `git2`.
2. Transport/auth code compiles with `gix`-first inputs except where a deliberate backend adapter still needs `git2`.
3. `gitbutler-tauri` and `but-napi` no longer open or thread `git2` repositories except for the accepted hard boundary.

Current reconciliation notes:

- `crates/gitbutler-repo/src/logging.rs` and `crates/gitbutler-project/src/project.rs` are no longer current migration targets; they do not have direct `git2::` usage in the reconciled tree.
- Remaining repo-adapter and transport cleanup is concentrated in `crates/gitbutler-repo/src/repository_ext.rs`, `crates/gitbutler-repo/src/commands.rs`, `crates/gitbutler-repo/src/rebase.rs`, `crates/gitbutler-repo/src/credentials.rs`, `crates/gitbutler-repo/src/hooks.rs`, `crates/gitbutler-repo/src/managed_hooks.rs`, `crates/gitbutler-repo/src/remote.rs`, `crates/gitbutler-repo/src/staging.rs`, and `crates/gitbutler-repo-actions/src/repository.rs`.
- Frontend/application entrypoints still thread `git2` through `Context` in `crates/gitbutler-tauri/src/projects.rs` and `crates/but-napi/src/lib.rs`.
- Transport/auth remains explicitly libgit2-backed in `crates/gitbutler-repo/src/credentials.rs` and `crates/gitbutler-repo-actions/src/repository.rs`.

### Workstream E: Test Surface and Dependency Reduction

This workstream exists for behavioral isolation as well as dependency cleanup. Tests should prefer `gix`/`but-*` helpers because isolated repository opens are a first-class requirement for deterministic test behavior, and that isolation cannot be configured equivalently through `git2` in our current setup.

Target tests/modules:

- `crates/gitbutler-branch-actions/tests/branch-actions/squash.rs`
- `crates/gitbutler-branch-actions/tests/branch-actions/virtual_branches/mod.rs`
- `crates/gitbutler-branch-actions/tests/branch-actions/virtual_branches/move_commit_to_vbranch.rs`
- `crates/gitbutler-branch-actions/tests/branch-actions/virtual_branches/unapply_without_saving_virtual_branch.rs`
- `crates/gitbutler-edit-mode/tests/edit_mode.rs`
- `crates/gitbutler-stack/tests/mod.rs`
- `crates/gitbutler-repo/tests/create_wd_tree.rs`
- `crates/gitbutler-repo/tests/managed_hooks_tests.rs`
- `crates/gitbutler-project/tests/project/main.rs`
- `crates/but-testsupport/src/legacy.rs`
- `crates/but-testsupport/src/legacy/suite.rs`
- `crates/but-testsupport/src/legacy/test_project.rs`
- `crates/but-testsupport/src/legacy/testing_repository.rs`

Tasks:

1. Update tests to `gix` IDs where migrated paths changed.
2. Continue moving test helpers to `but-testsupport` and stop expanding `gitbutler-testsupport`; the end state is to replace `gitbutler-testsupport` with `but-testsupport` so `gitbutler-testsupport` can be deleted.
3. Rewrite direct test assertions/setup that still use `ctx.git2_repo` when an equivalent `gix`/helper path is available.
4. Keep only hard-boundary tests/fixtures on `git2`; migrate the rest as production code moves, especially where isolation-sensitive setup can move to `gix`-based helpers.
5. Remove `git2` from crate manifests only after last in-scope reference is gone.

Acceptance:

1. Workspace test suite compiles and passes relevant targets.
2. Cargo manifests show reduced `git2` dependencies in migrated crates.
3. Routine test authorship no longer requires `ctx.git2_repo` for non-excluded scenarios.
4. Isolation-sensitive tests and fixture helpers use `gix`/`but-*` paths by default instead of `git2` repository opens.

Current reconciliation notes:

- Actionable manifest cleanup is not complete yet.
- `gitbutler-branch` no longer depends on `git2` directly.
- `cargo metadata` still reports 18 crates with a normal `git2` dependency, so manifest cleanup remains blocked on production and test/helper cleanup.
- `crates/gitbutler-stack/tests/mod.rs` now bakes stack commit history via `gix` traversal instead of `ctx.git2_repo`.
- `crates/gitbutler-branch-actions/tests/branch-actions/virtual_branches/workspace_migration.rs` now inspects HEAD through `gix`.
- On 2026-03-21, the shared legacy helper surface (`Case`, `Suite`, `TestProject`, `testing_repository`, `secrets`, `paths`, `stack_details`, and related setup helpers) was moved into `crates/but-testsupport/src/legacy/*`, and the current test users in `gitbutler-operating-modes`, `gitbutler-project`, `gitbutler-repo`, and `gitbutler-branch-actions` were switched from `gitbutler-testsupport` to `but-testsupport` with the `legacy` feature.
- `crates/gitbutler-project/tests/project/main.rs` and `crates/gitbutler-oplog/src/reflog.rs` no longer contain direct `git2` usage.
- `crates/gitbutler-edit-mode/tests/edit_mode.rs`, `crates/gitbutler-repo/tests/create_wd_tree.rs`, `crates/gitbutler-repo/tests/managed_hooks_tests.rs`, and `crates/but-testsupport/src/legacy/*` still contain substantial direct `git2` usage.
- On 2026-03-21, the `gitbutler-testsupport` shim crate was deleted after the remaining users had moved to `but-testsupport`.
- `crates/gitbutler-branch-actions/tests/branch-actions/squash.rs` is currently the largest remaining `ctx.git2_repo` consumer in tests.
- Remaining direct `ctx.git2_repo` usage in tests should trend toward hard-boundary coverage and fixture/setup helpers only as reopened production migrations land.

### Workstream F: Legacy Context Boundary Deprecation

Target modules:

- `crates/but-ctx/src/lib.rs`
- all remaining callers of `ctx.git2_repo`

Tasks:

1. After Workstream E narrows test/runtime `git2` usage to intentional boundaries, add a deprecation annotation to `Context::git2_repo`.
2. Add targeted `#[expect(deprecated)]` only at accepted legacy/excluded boundary call-sites so the warning stays useful.
3. Document the allowed reasons to use `ctx.git2_repo`: the exact checkout/materialization and tree-creation hard boundary, plus intentional compatibility/public-boundary code only.

Acceptance:

1. Accessing `ctx.git2_repo` emits a deprecation warning by default.
2. Existing accepted call-sites compile cleanly via explicit local allowance.
3. New `git2` introductions through `ctx.git2_repo` become mechanically discouraged.

Current reconciliation notes:

- `crates/but-ctx/src/lib.rs` now marks `Context::git2_repo` as deprecated and documents it as a boundary-only escape hatch.
- Direct `ctx.git2_repo` / `with_git2_repo` call-sites are down to 26 across 11 files, and every accepted residual caller is annotated locally.
- Accepted residual `ctx.git2_repo` callers are limited to checkout/index materialization (`crates/gitbutler-workspace/src/branch_trees.rs`, `crates/gitbutler-edit-mode/src/lib.rs`, `crates/gitbutler-oplog/src/oplog.rs`, `crates/gitbutler-branch-actions/src/base.rs`, `crates/gitbutler-branch-actions/src/branch_manager/branch_removal.rs`, `crates/gitbutler-branch-actions/src/integration.rs`), hook adapters and hook coverage (`crates/gitbutler-repo/src/hooks.rs`, `crates/gitbutler-branch-actions/tests/branch-actions/hooks.rs`), deliberate compatibility helpers (`crates/but-testsupport/src/legacy/mod.rs`, `crates/gitbutler-repo/tests/credentials.rs`), and edit-mode boundary coverage (`crates/gitbutler-edit-mode/tests/edit_mode.rs`).
- Workstream F is complete for the current scope.

## Ordered Execution Waves

1. Wave 1: Foundational types/serde (`but-serde`, `but-ctx`, shared author/signature paths).
2. Wave 2: Config and refname path (`but` alias/config and `gitbutler-reference`).
3. Wave 3: Branch/cherry-pick legacy logic conversion (non-excluded functions only).
4. Wave 4: Oplog metadata/state migration.
5. Wave 5: Reopen mixed legacy modules, starting with `gitbutler-edit-mode`, workspace/materialization callers, and `gitbutler-oplog`.
6. Wave 6: Repo adapter and transport/auth cleanup (`gitbutler-repo*`, `gitbutler-project`, `gitbutler-tauri`, stack push helpers).
7. Wave 7: Tests and manifest cleanup.
8. Wave 8: Final `ctx.git2_repo` tightening after boundary narrowing is complete.

1. Reconcile this migration document to the current tree and validation baseline.
2. Remove eager `git2` cache population from activation entrypoints and tighten `Context::git2_repo` documentation.
3. Refactor workspace materialization helpers to expose `gix`-first interfaces while keeping checkout/index work behind narrow `git2` adapters.
4. Isolate the remaining `git2` handoff in `gitbutler-edit-mode` and `gitbutler-oplog`.
5. Convert repo-facing helpers to `gix`-first APIs and keep `git2` transport/auth and hook/index code behind explicit adapters.
6. Remove non-boundary compatibility/test callers and shrink the residual `git2` surface to explicit adapters and hard boundaries.
7. Deprecate `Context::git2_repo`, add local `#[expect(deprecated)]` only at accepted residual boundary/compatibility sites, and reconcile this document to the final caller inventory.

Each patch in the series must finish with:

- compile checks for touched crates
- migration audit command output updated
- explicit record of residual `git2` usage and why

Current execution status:

- Workstreams A and B are largely complete.
- Workstreams C, D, E, and G remain active under the narrowed hard boundary.
- Workstream F has not started yet.
- Residual `git2` usage is not yet limited to the hard boundary and compatibility/public-boundary crates.
- Open actionable items: repo adapter cleanup, transport/auth cleanup, workspace/oplog boundary extraction, and continued test/helper `git2` reduction inside `but-testsupport::legacy`.
- Recommended next wave: Wave 5, starting with `crates/gitbutler-workspace/src/branch_trees.rs`, `crates/gitbutler-oplog/src/oplog.rs`, and cleanup of the remaining `git2`-heavy helpers/tests that were just consolidated into `but-testsupport`.

## Acceptance Criteria

Global completion criteria:

1. All `git2` callsites outside the exact checkout/materialization and tree-creation hard boundary are removed or boundary-isolated.
2. Residual `git2` use is only in the explicit hard boundary, compatibility/public-boundary crates, or deliberately isolated backend adapters.
3. No API/schema regressions for ID/time serialization.
4. Test and fixture setup no longer depends on `git2` where isolated repository access is required.
5. CI-level checks continue to pass.

Recommended verification commands:

```bash
cargo clippy --all-targets --workspace
```

Migration audits:

```bash
rg -n "git2::" crates -S --glob '*.rs'
```

Plus a filtered report that excludes only:

- the exact checkout/worktree materialization boundary
- tree creation from worktree/index state
- explicit compatibility/public-boundary helper crates that are being retired separately

The filtered report must trend to zero in-scope matches.

## Current Residual git2 Inventory

Remaining direct `git2` usage after reconciliation is split between hard-boundary code and still-actionable legacy adapters:

- Hard-boundary and boundary-adjacent runtime code: `crates/but-core/src/worktree/checkout/*`, `crates/gitbutler-workspace/src/branch_trees.rs`, `crates/gitbutler-edit-mode/src/lib.rs` (treated as a boundary crate for its remaining checkout/index handoff), `crates/gitbutler-oplog/src/oplog.rs`, and the `git2` index/reset portions of `crates/gitbutler-branch-actions/src/integration.rs`
- Actionable repo/transport/frontend adapters: `crates/gitbutler-repo/src/repository_ext.rs`, `crates/gitbutler-repo/src/commands.rs`, `crates/gitbutler-repo/src/rebase.rs`, `crates/gitbutler-repo/src/credentials.rs`, `crates/gitbutler-repo/src/hooks.rs`, `crates/gitbutler-repo/src/remote.rs`, `crates/gitbutler-repo/src/staging.rs`, and `crates/gitbutler-repo-actions/src/repository.rs`
- Foundational/shared compatibility boundaries still to shrink once callers move: `crates/but-ctx/src/lib.rs`, `crates/but-oxidize/src/lib.rs`, `crates/but-serde/src/lib.rs`, `crates/but-schemars/src/lib.rs`, `crates/gitbutler-project/src/lib.rs`, `crates/gitbutler-repo/src/lib.rs`, `crates/gitbutler-cherry-pick/src/*`, `crates/gitbutler-commit/src/commit_ext.rs`, and the remaining `git2` type boundary in `crates/gitbutler-stack/src/stack.rs`
- Legacy test and fixture/setup code that still constructs `git2` values directly: selected `crates/gitbutler-branch-actions/tests/*`, `crates/gitbutler-edit-mode/tests/edit_mode.rs`, `crates/gitbutler-repo/tests/*`, and `crates/but-testsupport/src/legacy/*`
- Planned helper retirement: keep shrinking `git2` inside `but-testsupport::legacy` until the legacy feature itself can collapse away.

Residual `git2` usage that is currently consistent with the narrowed hard boundary:

- Checkout execution and worktree materialization
- Index tree materialization from staged state and its immediate adapters
- `ctx.git2_repo` call-sites that remain only to bridge into those exact boundary paths or compatibility/public-boundary code

## Risks and Controls

1. Mixed modules (in-scope and excluded code together) can cause churn.
   - Control: isolate excluded behavior in adapters first, then migrate surrounding logic.
2. Serialization regressions on OID/time.
   - Control: fixture-based roundtrip tests before/after each wave.
3. Legacy-heavy crates may require staged type aliases.
   - Control: temporary compatibility aliases with strict TODO removal markers.

## Explicit Assumptions

1. The only strict hard boundary for this effort is checkout/worktree materialization and tree creation from worktree/index state.
2. Dual transport backend remains as-is, but transport/auth `git2` usage is still in-scope for cleanup and narrowing.
3. Single-file plan artifact is the source of truth for execution tracking.

## Tracking Checklist

Use this section as the running checklist during implementation.

- [x] Historical wave 1 complete
- [x] Historical wave 2 complete
- [x] Patch 1 complete: plan reconciliation
- [x] Patch 2 complete: activation/context cleanup
- [x] Patch 3 complete: workspace boundary extraction
- [x] Patch 4 complete: edit-mode/oplog isolation
- [x] Patch 5 complete: repo adapter cleanup
- [x] Patch 6 complete: compatibility surface cleanup
- [x] Patch 7 complete: test cleanup and `ctx.git2_repo` deprecation
- [ ] In-scope `git2` audit at zero
- [x] Residual `git2` inventory documented
- [x] `ctx.git2_repo` deprecation landed
- [x] Residual `git2` usage is hard-boundary or explicit compatibility-adapter only
