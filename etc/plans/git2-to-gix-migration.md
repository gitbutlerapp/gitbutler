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

Current raw audit on 2026-03-12:

- `git2::` callsites: 450
- files with `git2::` references: 68
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
- Direct `git2` OID/commit usage still remains in `move_commits.rs`, `virtual.rs`, `gitbutler-cherry-pick`, `gitbutler-commit`, `gitbutler-edit-mode/src/lib.rs`, and several `gitbutler-branch-actions` modules that were missing from the original target list.
- Stack/workspace legacy surfaces still carry `git2` through commit logging, merge-base traversal, workspace-head creation, and branch/target helpers in `but-workspace::legacy`, `gitbutler-stack`, and `but-api::legacy::virtual_branches`.

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

Remaining in-scope surfaces confirmed during reconciliation:

- Foundational/shared boundaries: `crates/but-ctx/src/lib.rs`, `crates/but-serde/src/lib.rs`, `crates/but-schemars/src/lib.rs`, `crates/gitbutler-project/src/lib.rs`, `crates/gitbutler-repo/src/lib.rs`
- Branch/stack/workspace logic: `crates/gitbutler-branch-actions/src/*`, `crates/gitbutler-cherry-pick/src/*`, `crates/gitbutler-commit/src/commit_ext.rs`, `crates/gitbutler-edit-mode/src/lib.rs`, `crates/but-workspace/src/legacy/{head,integrated,stacks}.rs`, `crates/gitbutler-stack/src/{stack,stack_branch,state,target}.rs`, `crates/but-api/src/legacy/virtual_branches.rs`
- Test follow-up: `crates/gitbutler-branch-actions/tests/*`, `crates/gitbutler-operating-modes/tests/operating_modes.rs`, and remaining legacy fixture/setup tests that still construct `git2` IDs directly

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

### 2026-03-12

- Reconciled the plan against `next3`, including the incoming `but move` command merge, while preserving the `gix`-native commit ID changes already made on this branch.
- The branch is effectively in Wave 3, not executing the waves strictly in order:
  - Wave 2 is complete at the code level.
  - Wave 4 is complete for the planned non-excluded oplog metadata/state scope.
  - Wave 1 is still open because foundational compatibility boundaries remain in `crates/but-ctx/src/lib.rs`, `crates/but-serde/src/lib.rs`, `crates/but-schemars/src/lib.rs`, `crates/gitbutler-project/src/lib.rs`, and `crates/gitbutler-repo/src/lib.rs`.
  - Wave 5 has started opportunistically through manifest cleanup, but remains open because test surfaces still retain `git2`-native setup/assertions.
- Fresh raw audit now shows `450` `git2::` callsites across `68` Rust files; the file count is unchanged from the prior checkpoint, so Wave 3 remains the active migration focus.
- Most recently verified cleanup:
  - removed unused manifest dependencies from `crates/but-oplog/Cargo.toml`, `crates/gitbutler-operating-modes/Cargo.toml`, and `crates/gitbutler-reference/Cargo.toml`
  - resolved the `crates/but/src/command/commit/move.rs` merge by taking the incoming command behavior while keeping `gix::ObjectId`-native move paths and the consolidated `commit::move` implementation
  - reran `cargo fmt --check --all`, `cargo machete`, and `cargo check -p but-action -p gitbutler-branch-actions -p but-oplog -p gitbutler-reference -p gitbutler-operating-modes`
- Remaining in-scope work is still concentrated in the Wave 3 and Wave 1 surfaces already listed in this document; no evidence yet that the in-scope residual inventory is down to excluded-boundary-only usage.

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
