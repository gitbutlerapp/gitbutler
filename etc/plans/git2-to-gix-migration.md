# git2 to gix Migration Plan (Single File, Scoped)

## Summary

This plan orchestrates migration from `git2` to `gix` across the codebase where possible, while explicitly excluding:

- `push/pull/fetch` migration work
- worktree `reset/checkout` migration work
- `index -> tree` migration work

Transport strategy remains dual-backend for now (no transport backend rewrite in this effort).

## Baseline and Scope

Repository scan baseline at plan creation:

- `git2::` callsites: 566
- files with `git2::` references: 88
- files currently in-scope under exclusions: 35

Current raw audit on 2026-03-15:

- `git2::` callsites: 357
- files with `git2::` references: 51
- filtered in-scope audit: not yet zero

In-scope means all non-excluded `git2` usage should be migrated to `gix` or isolated behind explicit adapter boundaries.

Out-of-scope means no behavioral rewrite or backend switch in:

- push/fetch/pull flows
- checkout/reset flows
- index-to-tree flows (`create_wd_tree`, `write_tree/read_tree`, related)

## Migration Goals

1. Eliminate non-excluded direct `git2` usage from application/business logic.
2. Migrate public/internal data models from `git2` IDs/time to `gix` IDs/time where not blocked by exclusions.
3. Preserve JSON/TOML wire formats and behavior.
4. Reduce crate-level `git2` dependencies where no longer needed.

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
- `crates/but-ctx/src/lib.rs`, `crates/gitbutler-project/src/lib.rs`, and `crates/gitbutler-repo/src/lib.rs` still expose legacy `git2` repository/signature boundaries.
- `crates/but-serde/src/lib.rs` and `crates/but-schemars/src/lib.rs` still retain legacy `git2` helpers for compatibility.

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
- Wave 2 is complete at the code level; remaining work here is only regression coverage and eventual dependency cleanup.

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

- `crates/gitbutler-branch-actions/src/author.rs`, `crates/gitbutler-branch-actions/src/hooks.rs`, and `crates/gitbutler-branch-actions/src/remote.rs` are already `gix`-first or boundary-only.
- `crates/but/src/command/legacy/branch/list.rs` and `crates/but/src/command/legacy/branch/show.rs` are partially migrated to `gix::ObjectId`; remaining `git2` there is concentrated in legacy diff inspection and merge-query edges.
- Direct `git2` OID/commit usage still remains in `move_commits.rs`, `virtual.rs`, `gitbutler-cherry-pick`, `gitbutler-commit`, `gitbutler-edit-mode/src/lib.rs`, and several `gitbutler-branch-actions` modules that were missing from the original target list.
- `crates/gitbutler-stack/src/stack_branch.rs` now computes local/remote/upstream commit membership with `gix` traversals and keeps `git2` only at the legacy commit-return boundary; `crates/gitbutler-branch-actions/src/reorder.rs` and `crates/but-workspace/src/legacy/stacks.rs` now consume the `gix`-first path directly.
- Stack/workspace legacy surfaces still carry `git2` through workspace-head creation, push/transport helpers, and branch/target adapters in `gitbutler-stack`, `gitbutler-branch-actions`, and `but-api::legacy::virtual_branches`.

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
3. Keep excluded checkout/reset/index-tree operations unchanged and boundary-adapted.

Acceptance:

1. Existing snapshot metadata loads and writes without schema regression.
2. Oplog tests pass with migrated non-excluded types.

Current reconciliation notes:

- Snapshot metadata/state surfaces in `entry.rs`, `snapshot.rs`, `state.rs`, and `tests/oplog/main.rs` are migrated to `gix`.
- `crates/gitbutler-oplog/src/oplog.rs` still uses `git2` in restore/diff/prepare helpers that cross excluded checkout/reset/index-tree boundaries.

### Workstream E: Test Surface and Dependency Reduction

Target tests/modules:

- `crates/gitbutler-branch-actions/tests/squash.rs`
- `crates/gitbutler-branch-actions/tests/virtual_branches/mod.rs`
- `crates/gitbutler-branch-actions/tests/virtual_branches/move_commit_to_vbranch.rs`
- `crates/gitbutler-branch-actions/tests/virtual_branches/unapply_without_saving_virtual_branch.rs`
- `crates/gitbutler-operating-modes/tests/operating_modes.rs`

Tasks:

1. Update tests to `gix` IDs where migrated paths changed.
2. Keep excluded-domain tests untouched unless required for compilation.
3. Remove `git2` from crate manifests only after last in-scope reference is gone.

Acceptance:

1. Workspace test suite compiles and passes relevant targets.
2. Cargo manifests show reduced `git2` dependencies in migrated crates.

## Ordered Execution Waves

1. Wave 1: Foundational types/serde (`but-serde`, `but-ctx`, shared author/signature paths).
2. Wave 2: Config and refname path (`but` alias/config and `gitbutler-reference`).
3. Wave 3: Branch/cherry-pick legacy logic conversion (non-excluded functions only).
4. Wave 4: Oplog metadata/state migration.
5. Wave 5: Tests and manifest cleanup.

Each wave must finish with:

- compile checks for touched crates
- migration audit command output updated
- explicit record of residual `git2` usage and why

Current execution status:

- Active wave: Wave 3
- Completed waves: Wave 2, Wave 4
- Still open: Wave 1 foundational boundary cleanup, Wave 3 legacy domain conversion, Wave 5 tests and manifest cleanup

## Acceptance Criteria

Global completion criteria:

1. All non-excluded `git2` callsites are removed or boundary-isolated.
2. Residual `git2` use is only in explicitly excluded domains.
3. No API/schema regressions for ID/time serialization.
4. CI-level checks continue to pass.

Recommended verification commands:

```bash
cargo clippy --all-targets --workspace
```

Migration audits:

```bash
rg -n "git2::" crates -S --glob '*.rs'
```

Plus a filtered report that excludes:

- transport (`push/fetch/pull`)
- checkout/reset
- index-to-tree

The filtered report must trend to zero in-scope matches.

## Current Residual git2 Inventory

Remaining direct `git2` usage after reconciliation is concentrated in explicit boundary layers:

- Foundational/shared compatibility boundaries: `crates/but-ctx/src/lib.rs`, `crates/but-serde/src/lib.rs`, `crates/but-schemars/src/lib.rs`, `crates/gitbutler-project/src/lib.rs`, `crates/gitbutler-repo/src/lib.rs`
- Legacy adapter shims around still-supported `git2` callers: `crates/gitbutler-cherry-pick/src/*`, `crates/gitbutler-commit/src/commit_ext.rs`, `crates/gitbutler-stack/src/stack_branch.rs`, `crates/gitbutler-stack/src/stack.rs`, `crates/gitbutler-stack/src/state.rs`, `crates/gitbutler-branch-actions/src/base.rs`, and `crates/gitbutler-branch-actions/src/stack.rs`
- Test follow-up and fixture/setup code that still constructs `git2` values directly: `crates/gitbutler-branch-actions/tests/*`, `crates/gitbutler-stack/tests/mod.rs`, and related legacy integration tests

Residual `git2` usage that is currently consistent with explicit exclusions:

- Transport and remote-operation code paths, including fetch/push/pull and hook helpers
- Checkout/reset code paths, including edit-mode/workspace restore flows
- Index-to-tree and snapshot/workspace materialization helpers, including oplog restore/prepare internals and worktree tree creation

## Risks and Controls

1. Mixed modules (in-scope and excluded code together) can cause churn.
   - Control: isolate excluded behavior in adapters first, then migrate surrounding logic.
2. Serialization regressions on OID/time.
   - Control: fixture-based roundtrip tests before/after each wave.
3. Legacy-heavy crates may require staged type aliases.
   - Control: temporary compatibility aliases with strict TODO removal markers.

## Explicit Assumptions

1. Exclusions are strict for this effort:
   - no push/pull/fetch migration
   - no checkout/reset migration
   - no index->tree migration
2. Dual transport backend remains as-is.
3. Single-file plan artifact is the source of truth for execution tracking.

## Tracking Checklist

Use this section as the running checklist during implementation.

- [ ] Wave 1 complete
- [x] Wave 2 complete
- [ ] Wave 3 complete
- [x] Wave 4 complete
- [ ] Wave 5 complete
- [x] Active wave identified as Wave 3
- [ ] In-scope `git2` audit at zero
- [x] Residual `git2` inventory documented
- [ ] Residual `git2` usage is excluded-boundary only

## Progress Log

### 2026-03-15

- Reconciled this plan again against the current branch after the latest Wave 3 stack/workspace traversal cleanup.
- Fresh raw audit now shows `357` `git2::` callsites across `51` Rust files; the previous `381` across `54` files checkpoint from 2026-03-12 is stale.
- Additional Wave 3 progress on 2026-03-15:
  - `crates/gitbutler-stack/src/stack_branch.rs`: introduced a `gix`-first `commit_ids()` path for local, remote, and upstream-only branch commit selection, leaving `git2` only in the compatibility `commits()` return boundary for legacy callers and tests.
  - `crates/but-workspace/src/legacy/stacks.rs`: switched stack branch local/remote commit materialization to the new `gix` commit-ID path and removed direct `git2` author/time/parent traversal from that UI-facing flow.
  - `crates/gitbutler-branch-actions/src/reorder.rs`, `crates/gitbutler-branch-actions/tests/branch-actions/reorder.rs`, `crates/gitbutler-stack/tests/mod.rs`, and `crates/gitbutler-branch-actions/tests/branch-actions/squash.rs`: moved reorder/stack/squash test commit enumeration onto `gix::ObjectId`, leaving the touched test files with only unrelated fixture/blob helpers on `git2`.
  - raw audit on the touched production files now shows only the expected `git2` compatibility boundary in `crates/gitbutler-stack/src/stack_branch.rs`
  - verification for this slice: `cargo fmt --all`, `cargo check -p gitbutler-stack -p gitbutler-branch-actions -p but-workspace --all-targets`, `cargo test -p gitbutler-stack list_series_default_head -- --nocapture`, `cargo test -p gitbutler-branch-actions reorder_in_top_series -- --nocapture`, and `cargo test -p gitbutler-branch-actions squash_without_affecting_stack -- --nocapture` all pass

### 2026-03-12

- Reconciled the plan against `next3`, including the incoming `but move` command merge, while preserving the `gix`-native commit ID changes already made on this branch.
- The branch is effectively in Wave 3, not executing the waves strictly in order:
  - Wave 2 is complete at the code level.
  - Wave 4 is complete for the planned non-excluded oplog metadata/state scope.
  - Wave 1 is still open because foundational compatibility boundaries remain in `crates/but-ctx/src/lib.rs`, `crates/but-serde/src/lib.rs`, `crates/but-schemars/src/lib.rs`, `crates/gitbutler-project/src/lib.rs`, and `crates/gitbutler-repo/src/lib.rs`.
  - Wave 5 has started opportunistically through manifest cleanup, but remains open because test surfaces still retain `git2`-native setup/assertions.
- Fresh raw audit now shows `448` `git2::` callsites across `61` Rust files after the latest Wave 3 API-boundary cleanup; the previous `438` callsite count should be treated as a stale checkpoint rather than the current branch state.
- Fresh raw audit now shows `387` `git2::` callsites across `54` Rust files after the latest Wave 3 and Wave 5 follow-up cleanup; the previous `448` and `438` checkpoints are stale.
- Fresh raw audit now shows `381` `git2::` callsites across `54` Rust files after the latest legacy CLI follow-up; the previous `387`, `448`, and `438` checkpoints are stale.
- Fresh raw audit now shows `380` `git2::` callsites across `54` Rust files after the latest branch-actions traversal cleanup; the previous `381`, `387`, `448`, and `438` checkpoints are stale.
- Fresh raw audit now shows `381` `git2::` callsites across `54` Rust files on the current branch state after subsequent rounds; the previous `380`, `387`, `448`, and `438` checkpoints are stale.
- Most recently verified cleanup:
  - removed unused manifest dependencies from `crates/but-oplog/Cargo.toml`, `crates/gitbutler-operating-modes/Cargo.toml`, and `crates/gitbutler-reference/Cargo.toml`
  - resolved the `crates/but/src/command/commit/move.rs` merge by taking the incoming command behavior while keeping `gix::ObjectId`-native move paths and the consolidated `commit::move` implementation
  - `crates/but/src/command/legacy/branch/list.rs`: migrated target/tip filtering and ahead-of-target emptiness checks to `gix::ObjectId`, leaving `git2` only at the remaining legacy commit lookup and merge-check boundaries
  - `crates/but/src/command/legacy/branch/show.rs`: changed merge-conflict tree collection helpers to take `gix::ObjectId` directly instead of passing `git2::Oid` through a `gix`-only merge path
  - reran `cargo fmt --check --all`, `cargo machete`, and `cargo check -p but-action -p gitbutler-branch-actions -p but-oplog -p gitbutler-reference -p gitbutler-operating-modes`
- Additional Wave 3 progress on 2026-03-12:
  - `crates/gitbutler-branch-actions/src/actions.rs`: made `squash_commits()` and `update_commit_message()` gix-first at the public API boundary, converting to `git2` only at the remaining legacy implementation edge
  - `crates/but-api/src/legacy/virtual_branches.rs`, `crates/but-action/src/reword.rs`, `crates/but-tools/src/workspace.rs`, and `crates/but/src/command/legacy/rub/squash.rs`: removed now-unnecessary `to_git2()`/`to_gix()` plumbing around those branch-actions entry points
  - `crates/gitbutler-branch-actions/src/actions.rs`: made `amend()` gix-first at the public API boundary while preserving `git2` compatibility for `gitbutler-` crate tests
  - `crates/gitbutler-branch-actions/src/virtual.rs`, `crates/gitbutler-branch-actions/src/upstream_integration.rs`, and `crates/gitbutler-branch-actions/src/move_commits.rs`: converted additional internal Wave 3 helpers from `git2` commit/OID state to `gix::ObjectId`, leaving `git2` only at remaining legacy repository and serialized-input boundaries
  - `crates/but-oxidize/src/lib.rs`: added `IntoGixObjectId` so production APIs can accept both `gix::ObjectId` and legacy `git2::Oid` without forcing `gitbutler-` crate tests off `git2`
  - `crates/gitbutler-branch-actions/src/squash.rs`, `crates/gitbutler-branch-actions/src/integration.rs`, `crates/gitbutler-branch-actions/src/branch_manager/branch_creation.rs`, and `crates/but-workspace/src/legacy/head.rs`: removed more internal `git2` OID/commit plumbing and converted more intermediate state to `gix::ObjectId`
  - `crates/gitbutler-stack/src/stack.rs`: made stack tree/head update helpers `gix`-first internally, keeping `git2` only where push compatibility and legacy validation boundaries still require it
  - `crates/gitbutler-stack/src/state.rs` and `crates/but-workspace/src/legacy/integrated.rs`: moved more stack/workspace merge-base and upstream-integration helpers onto `gix` repository APIs
  - `crates/gitbutler-branch-actions/src/{branch,reorder,squash,actions}.rs` and `crates/but/src/command/{commit/move.rs,legacy/{branch,pick,show, rub/squash}.rs}`: aligned the CLI and branch-action surfaces with `gix::ObjectId`-native branch heads and reordered commit lists
  - `crates/gitbutler-branch-actions/tests/reorder.rs`: migrated the reordered commit assertions to `gix::ObjectId`
  - `crates/gitbutler-edit-mode/src/lib.rs` and `crates/gitbutler-edit-mode/src/commands.rs`: converted edit-mode helper signatures from `git2::Commit`/`git2::Oid` to `gix::ObjectId`, leaving `git2` only at checkout/index repository boundaries
  - `crates/but-workspace/src/legacy/stacks.rs`: moved `CommitData` comparison logic to `gix::Commit`
  - `crates/gitbutler-oplog/src/oplog.rs`: adapted snapshot tree collection to the new `gix`-first stack tree helper without widening the existing excluded restore/checkout boundary
  - verification for this slice: `cargo fmt --check --all`, `cargo check --all-targets -p but -p but-api -p gitbutler-branch-actions`, and `cargo check --workspace --all-targets` all pass
- Additional Wave 3 follow-up on 2026-03-12:
  - `crates/but/src/command/legacy/branch/show.rs`: migrated target-branch resolution and merge-base/tree selection for merge checking onto `gix::ObjectId`, leaving `git2` only in the remaining commit-diff inspection path
  - `crates/but/src/command/legacy/show.rs`: removed redundant `git2` branch-head plumbing from branch commit listing and stack-chain counting by reusing the workspace merge-base result directly on `gix` IDs
  - `crates/gitbutler-stack/src/stack.rs`: replaced stack commit enumeration with a `gix` first-parent ancestor traversal and narrowed `push_details()` to a simple excluded-boundary `to_git2()` conversion
  - `crates/gitbutler-branch-actions/src/base.rs`: migrated base-branch divergence, upstream-commit, and recent-commit collection from legacy git2 revwalk helpers to `gix` first-parent traversals while preserving existing remote/config compatibility boundaries
  - verification for this slice: `cargo fmt --all`, `cargo clippy -p but -p gitbutler-branch-actions -p gitbutler-stack --all-targets`, `cargo test -p but --test but journey::from_workspace -- --nocapture`, `cargo test -p gitbutler-stack add_series_top_base -- --nocapture`, `cargo test -p gitbutler-stack push_series_success -- --nocapture`, and `cargo test -p gitbutler-stack update_name_after_push -- --nocapture` all pass
- Additional Wave 3 follow-up on 2026-03-12 (branch-actions continuation):
  - `crates/gitbutler-branch-actions/src/move_commits.rs`: removed the remaining git2-only commit existence and rebase-head plumbing so stack-to-stack commit moves now stay on `gix::ObjectId` end-to-end until the workspace update boundary
  - `crates/gitbutler-branch-actions/src/integration.rs`: migrated workspace-head cleanliness detection from legacy git2 revwalk helpers to a `gix` first-parent traversal, keeping `git2` only for the explicit reset and commit-writing boundary
  - `crates/but-api/src/legacy/virtual_branches.rs`: rechecked as part of this slice and confirmed it is already acting as a thin `gix`-native wrapper around branch-actions APIs, so no additional migration churn was needed there
  - raw audit remains `381` `git2::` callsites across `54` Rust files because the remaining direct `git2::` hits in these files are boundary-only rather than business-logic OID flow
  - verification for this slice: `cargo fmt --all`, `cargo clippy -p gitbutler-branch-actions --all-targets`, `cargo test -p gitbutler-branch-actions works_on_integration_branch -- --nocapture`, `cargo test -p gitbutler-branch-actions no_diffs -- --nocapture`, and `cargo test -p gitbutler-branch-actions virtual_branches::move_commit_to_vbranch::multiple_commits -- --nocapture` all pass
- Additional Wave 3 follow-up on 2026-03-12 (branch-actions traversal cleanup):
  - `crates/gitbutler-branch-actions/src/virtual.rs`: removed the remaining git2-backed upstream commit enumeration and branch membership checks from `IsCommitIntegrated` and `update_commit_message()`, replacing them with direct `gix` ancestor traversal while keeping diff/push compatibility elsewhere unchanged
  - `crates/gitbutler-branch-actions/src/upstream_integration.rs`: resolved target-head discovery, merge-base calculation, and rebase commit selection with `gix` IDs/traversals, leaving `git2` only on the explicit merge-commit creation boundary
  - `crates/gitbutler-branch-actions/src/stack.rs`: updated `push_stack()` to use the narrowed `IsCommitIntegrated::new()` interface after the legacy `git2` repository dependency was removed from that helper
  - raw audit now shows `380` `git2::` callsites across `54` Rust files
  - verification for this slice: `cargo fmt --all`, `cargo clippy -p gitbutler-branch-actions --all-targets`, `cargo test -p gitbutler-branch-actions works_on_integration_branch -- --nocapture`, `cargo test -p gitbutler-branch-actions no_diffs -- --nocapture`, and `cargo test -p gitbutler-branch-actions update_commit_message -- --nocapture` all pass
- Additional Wave 3 follow-up on 2026-03-12 (`gitbutler-stack` traversal cleanup):
  - `crates/gitbutler-stack/src/stack_branch.rs`: replaced the legacy git2 `log()` and `revwalk()` commit collection in `StackBranch::commits()` with `gix` ancestor traversal for local, remote, and upstream-only commit selection
  - `crates/gitbutler-stack/src/stack_branch.rs`: kept the existing `BranchCommits` API stable by reopening the selected `gix::ObjectId`s as `git2::Commit` only at the return boundary for downstream callers and tests
  - raw audit remains `381` `git2::` callsites across `54` Rust files because this slice reduced legacy traversal logic without changing the remaining explicit `git2` type boundary in the file
  - verification for this slice: `cargo fmt --all`, `cargo clippy -p gitbutler-stack --all-targets`, `cargo test -p gitbutler-stack -- --nocapture`, and `cargo test -p gitbutler-branch-actions works_on_integration_branch -- --nocapture` all pass
- Remaining direct `git2` references are now concentrated in explicit compatibility adapters and excluded transport/checkout/index-tree paths rather than the previously broad Wave 3 business-logic surface.

### 2026-03-10

- Reconciled this plan against the current codebase.
- Raw audit currently shows `450` `git2::` callsites across `68` Rust files; the filtered in-scope audit is still non-zero.
- Confirmed completed work:
  - `crates/gitbutler-project/src/project.rs`: `CodePushState.id` migrated from `git2::Oid` to `gix::ObjectId`
  - `crates/but-workspace/src/ui/author.rs`: author conversion now uses `gix` signatures
  - `crates/gitbutler-operating-modes/src/lib.rs`: `EditModeMetadata.commit_oid` migrated to `gix::ObjectId`
  - `crates/gitbutler-oplog/src/entry.rs`, `crates/gitbutler-oplog/src/snapshot.rs`, and `crates/gitbutler-oplog/src/state.rs`: snapshot metadata/state migrated to `gix`
  - `crates/but/src/command/alias.rs` and `crates/gitbutler-reference/src/refname/*`: direct `git2` usage removed
  - `crates/but/src/command/config.rs` and `crates/but-api/src/legacy/git.rs`: migrated remaining config editing/inspection from `git2::Config` to `gix::config::File` and `config_snapshot()`
- Confirmed remaining in-scope work:
  - Wave 1 remains open because `crates/but-ctx/src/lib.rs` still owns `git2_repo`, `crates/gitbutler-project/src/lib.rs` still exposes `configure_git2()`, and `crates/gitbutler-repo/src/lib.rs` still returns `git2::Signature`
  - Wave 3 remains open across `gitbutler-branch-actions`, `gitbutler-cherry-pick`, `gitbutler-commit`, `but-workspace::legacy`, `gitbutler-stack`, `but-api::legacy::virtual_branches`, and `gitbutler-edit-mode/src/lib.rs`
  - Wave 5 remains open because targeted tests still construct/assert `git2` values and manifest cleanup has not happened
- Residual excluded-boundary `git2` use intentionally remains in:
  - transport and hook paths
  - checkout/reset paths
  - index-to-tree and related restore/workspace tree helpers
- Verification notes for this step:
  - `cargo check -p but-api --all-targets` passes after the config migration
  - `cargo check -p but --all-targets` was temporarily blocked by a `squash.rs`/`restore_snapshot()` ID mismatch during Wave 3 work
- Additional Wave 3 progress:
  - `crates/gitbutler-branch-actions/src/squash.rs`: fixed the `create_snapshot()`/`restore_snapshot()` boundary to convert the snapshot ID explicitly at the remaining legacy oplog edge
  - `crates/gitbutler-stack/src/target.rs`: migrated `Target::remote_head()` from `git2::Repository` to `gix::Repository`
  - `crates/gitbutler-stack/src/stack_branch.rs`: migrated `StackBranch::pushed()` to a `gix::Repository` boundary and updated its internal caller
  - `crates/but-api/src/legacy/virtual_branches.rs`: removed a leftover `git2::Oid::to_string()` wrapper in upstream-integration output
  - verification for this slice: `cargo check -p gitbutler-stack --all-targets`, `cargo check -p gitbutler-branch-actions --all-targets`, and `cargo check -p but-api --all-targets` all pass
  - `crates/gitbutler-branch-actions/src/virtual.rs` and `crates/but-workspace/src/legacy/integrated.rs`: migrated `is_integrated()` to take `gix::ObjectId` instead of `git2::Commit`
  - corresponding callers in `crates/gitbutler-branch-actions/src/stack.rs` and `crates/but-workspace/src/legacy/stacks.rs` now stay on gix IDs until the last necessary legacy boundary
  - verification for this slice: `cargo check -p but-workspace --all-targets` and `cargo check -p gitbutler-branch-actions --all-targets` both pass
  - `crates/gitbutler-stack/src/stack.rs`: migrated `Stack::commits()` and `commits_with_merge_base()` from `git2::Oid` to `gix::ObjectId`
  - `crates/gitbutler-branch-actions/src/stack.rs`: updated gerrit push metadata to consume the gix-native stack commit list directly
  - verification for this slice: `cargo check -p gitbutler-stack --all-targets`, `cargo check -p gitbutler-branch-actions --all-targets`, and `cargo check -p but-tools --all-targets` all pass
  - `crates/but-action/src/simple.rs` and `crates/but-api/src/legacy/oplog.rs`: removed stale `to_gix()` conversions now that `create_snapshot()` returns `gix::ObjectId`
  - `crates/but/src/command/config.rs` and `crates/but-api/src/legacy/git.rs`: aligned `gix` config editing helpers with the current `gix-config` API and removed the remaining direct `git2::Config` callers
  - `crates/but/src/lib.rs`: switched the TUI config probe to `ctx.repo.get()?.config_snapshot()` instead of `git2::Config`
  - verification for this slice: `cargo clippy --all-targets --workspace` passes
  - `crates/but-workspace/src/legacy/head.rs`: moved `remerged_workspace_tree_v2()` and `remerged_workspace_commit_v2()` to `gix::ObjectId` return types while keeping the final legacy commit creation isolated at the `git2` boundary
  - `crates/gitbutler-branch-actions/src/{base,integration.rs,branch_manager/mod.rs}`: updated clean callers to consume the new `gix`-first workspace-head/tree helpers with explicit conversion only where the legacy `git2` API still requires it
  - verification for this slice: `cargo clippy --all-targets --workspace` passes with raw audit reduced to `450` `git2::` callsites
