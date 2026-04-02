# git2 to gix Migration Plan

## Status

Most of the broad migration is already done. This document only tracks the remaining scope needed to finish, plus the intentional `git2` boundaries that are still allowed.

Repository audit on 2026-04-02:

- `git2::` callsites in `crates/**/*.rs`: 146
- files with `git2::` references: 24
- `ctx.git2_repo` / `ctx.with_git2_repo` callsites: 23

## Allowed Remaining Boundary

`git2` is still acceptable only where we do not yet have a practical `gix` replacement or where the code is deliberately acting as a compatibility adapter:

- checkout execution and worktree materialization
- index/tree materialization from staged state
- explicit transport/auth adapters that still rely on libgit2
- narrow compatibility surfaces that exist only to bridge older code

Anything outside those areas should continue moving to `gix`.

## Remaining Work

The remaining work is concentrated in a small set of areas:

### Workspace and edit-mode boundary cleanup

These modules still sit on the main checkout/index boundary and should be kept narrow:

- `crates/gitbutler-workspace/src/branch_trees.rs`
- `crates/gitbutler-edit-mode/src/lib.rs`
- `crates/gitbutler-oplog/src/oplog.rs`
- boundary portions of `crates/gitbutler-branch-actions/src/integration.rs`

Goal:

- keep `git2` usage isolated to the actual checkout/index handoff
- move surrounding read-side and domain logic to `gix` where possible

### Repo and transport adapters

These files still appear to contain actionable non-boundary `git2` usage or backend leakage:

- `crates/gitbutler-repo/src/repository_ext.rs`
- `crates/gitbutler-repo/src/commands.rs`
- `crates/gitbutler-repo/src/rebase.rs`
- `crates/gitbutler-repo/src/credentials.rs`
- `crates/gitbutler-repo/src/hooks.rs`
- `crates/gitbutler-repo/src/managed_hooks.rs`
- `crates/gitbutler-repo/src/remote.rs`
- `crates/gitbutler-repo/src/staging.rs`
- `crates/gitbutler-repo-actions/src/repository.rs`
- `crates/gitbutler-tauri/src/projects.rs`
- `crates/but-napi/src/lib.rs`

Goal:

- use `gix`-first APIs for repo reads and domain logic
- keep any remaining libgit2 use behind explicit transport/auth or hook adapters
- stop threading `git2` repositories through higher-level application code unless required by the accepted boundary

### Compatibility surfaces still to shrink

Some crates still intentionally expose `git2` compatibility helpers or legacy types:

- `crates/but-ctx/src/lib.rs`
- `crates/but-oxidize/src/lib.rs`
- `crates/but-serde/src/lib.rs`
- `crates/but-schemars/src/lib.rs`
- `crates/gitbutler-repo/src/lib.rs`
- `crates/gitbutler-cherry-pick/src/*`
- `crates/gitbutler-commit/src/commit_ext.rs`
- `crates/gitbutler-stack/src/stack.rs`

Goal:

- avoid expanding these surfaces
- remove or reduce them only when callers have moved off them

### Tests and legacy test support

Test-only `git2` usage still exists and should continue shrinking unless it is exercising the accepted hard boundary:

- selected `crates/gitbutler-branch-actions/tests/*`
- `crates/gitbutler-edit-mode/tests/edit_mode.rs`
- selected `crates/gitbutler-repo/tests/*`
- `crates/but-testsupport/src/legacy/*`

Goal:

- prefer `gix` and `but-*` helpers for fixtures and assertions
- keep direct `git2` setup only where boundary coverage actually requires it

## Current Direction

The intended end state is:

- `gix::ObjectId`, `gix` refs, config, and read-side repository access in normal application logic
- `Context::git2_repo` treated as a deprecated boundary escape hatch only
- residual `git2` usage limited to explicit hard-boundary or compatibility-adapter code

## Completion Criteria

This plan is complete when all of the following are true:

1. Non-boundary application logic no longer depends directly on `git2`.
2. Remaining `git2` use is confined to the accepted boundary or explicit compatibility/adapter code.
3. `ctx.git2_repo` callers are limited to those accepted sites.
4. Test and fixture code uses `gix` by default unless boundary coverage requires `git2`.
5. Validation still passes for touched crates and workspace-level checks.

## Verification

Recommended checks:

```bash
cargo clippy --all-targets --workspace
rg -n "git2::" crates -S --glob '*.rs'
rg -n "ctx\\.(git2_repo|with_git2_repo)" crates -S --glob '*.rs'
```

## Tracking

- [x] Broad migration completed
- [x] Config/refname migration completed
- [x] `Context::git2_repo` deprecated
- [ ] Workspace/edit-mode/oplog boundary reduced to the minimal handoff
- [ ] Repo and transport adapters narrowed further
- [ ] Test and legacy helper `git2` usage reduced to boundary coverage only
- [ ] In-scope `git2` audit at zero outside accepted boundaries
