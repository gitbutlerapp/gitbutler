# git2 to gix Migration Plan (Single File, Scoped)

## Summary

This plan orchestrates migration from `git2` to `gix` across the codebase where possible, while explicitly excluding:

- `push/pull/fetch` migration work
- worktree `reset/checkout` migration work
- `index -> tree` migration work

Transport strategy remains dual-backend for now (no transport backend rewrite in this effort).

## Baseline and Scope

Repository scan baseline:

- `git2::` callsites: 566
- files with `git2::` references: 88
- files currently in-scope under exclusions: 35

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

### Workstream B: Config and Refname Migration

Target modules:

- `crates/but/src/command/alias.rs`
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

### Workstream C: Legacy Domain OID/Commit Surface Cleanup (Non-Excluded)

Target modules:

- `crates/gitbutler-branch-actions/src/author.rs`
- `crates/gitbutler-branch-actions/src/hooks.rs`
- `crates/gitbutler-branch-actions/src/move_commits.rs`
- `crates/gitbutler-branch-actions/src/remote.rs`
- `crates/gitbutler-branch-actions/src/undo_commit.rs`
- `crates/gitbutler-branch-actions/src/virtual.rs`
- `crates/gitbutler-cherry-pick/src/lib.rs`
- `crates/gitbutler-cherry-pick/src/repository_ext.rs`
- `crates/gitbutler-commit/src/commit_ext.rs`
- `crates/but-workspace/src/legacy/integrated.rs`
- `crates/gitbutler-edit-mode/src/commands.rs`

Tasks:

1. Migrate function signatures and local structs from `git2` IDs to `gix` IDs where not crossing excluded paths.
2. Replace direct `git2` commit/tree access in non-excluded logic with `gix` equivalents.
3. For mixed files, isolate excluded operations behind narrow adapters and migrate surrounding logic.

Acceptance:

1. Non-excluded branch/cherry-pick/legacy-integrated flows compile with `gix`-first types.
2. Remaining `git2` in these modules is only for excluded-boundary interaction.

### Workstream D: Oplog Metadata and State Modernization (Non-Excluded)

Target modules:

- `crates/gitbutler-oplog/src/entry.rs`
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

## Acceptance Criteria

Global completion criteria:

1. All non-excluded `git2` callsites are removed or boundary-isolated.
2. Residual `git2` use is only in explicitly excluded domains.
3. No API/schema regressions for ID/time serialization.
4. CI-level checks continue to pass.

Recommended verification commands:

```bash
cargo check --workspace --all-targets
cargo check -p but-ctx --all-targets
cargo check -p but-serde --all-targets
cargo check -p but-workspace --all-targets
cargo check -p but-api --all-targets
cargo check -p but --all-targets
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
- [ ] Wave 2 complete
- [ ] Wave 3 complete
- [ ] Wave 4 complete
- [ ] Wave 5 complete
- [ ] In-scope `git2` audit at zero
- [ ] Residual `git2` list documented as excluded-boundary only
