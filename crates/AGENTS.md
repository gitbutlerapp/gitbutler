# Rust Instructions

## API Boundaries and Legacy

- `gitbutler-*` crates are legacy: don't add new uses of them from `but-*`
  code, and fix them in place rather than migrating.
- Minimize new `VirtualBranchesHandle` usage. At API boundaries, prefer
  `but_ctx::Context` workspace helpers and newer `but-*` APIs.
- `but-api` is the API surface for the Tauri app, the GPUI client, the CLI and the TUI.
  Outer callers should prefer existing `but-api` functions when they fit, but
  lower-level crates should not depend on `but-api`.
- Keep transport DTOs at the API boundary, commonly in a local `json` module,
  and convert them to domain types before calling lower-level crates.
- Lock the repository once, at the top-level API or command boundary, and pass
  the permission down: a permissioned `but-api` function is a wrapper that locks
  and a `_with_perm` implementation that takes the permission. While holding a
  guard, call only `_with_perm` or other permission-taking helpers; one that
  locks again deadlocks.
- Keep `Context` at API/composition boundaries; pass granular dependencies
  like repo, workspace, metadata, database handles, or an `Editor` into
  lower-level crates where possible.
- When an API offers both action-only and timeline-recording behavior, keep the
  `*_only*` function free of oplog side effects; prepare a best-effort oplog
  snapshot in the wrapper and commit it only after the mutation succeeds.
- New user-facing mutations should participate in undo: record an oplog
  snapshot via the existing `SnapshotDetails`/`OperationKind` wrapper pattern,
  and call out any mutation intentionally left out of the timeline rather than
  omitting it silently. Undo is snapshot-based; do not design bespoke inverse
  operations for it.
- Before changing or reviewing code that derives graph/workspace/branch/stack/commit
  relationships, reachability, dependencies, ordering, operation targets, or Git
  graph/history/ref-placement mutations, read `crates/WORKSPACE_MODEL.md`.

## Git Repository Semantics

- Do not rediscover Git repositories inside business logic; pass repo/context
  explicitly.
- Prefer repository APIs over shelling out to `git`, except at
  shell/executable boundaries such as hooks, debug tooling, tests, or Git
  interop helpers.
- Preserve Git graph semantics even when the shape looks redundant or odd; do
  not deduplicate, reorder, or smooth graph data unless Git semantics allow it.
- Use `gix` for new repository logic. Treat `git2` and `Context::git2_repo` as
  legacy/boundary-only escape hatches for libgit2 checkout/index, hooks,
  transport/auth, or existing code that cannot reasonably move yet.
- Keep Git paths, refnames, commit messages, and diff payloads byte-preserving
  until UI/API boundaries; avoid lossy `String` conversion in business logic.
- When frontend/API consumers need to classify an error, use existing
  `but_error::Code` patterns; do not make consumers match error strings.

## Database Migrations (`but-db`)

- Keep migrations forward-compatible: stay on the current `SchemaVersion` and
  leave columns or tables that new code stopped using in place. Bumping the
  version locks every older binary out of the database; reserve it for planned,
  coordinated breaks, never routine cleanup.

## Testing and Validation

- Make test repositories with `but-testsupport` scenarios, read-only ones for
  read-only behavior, never with `std::env::temp_dir()`.
- For graph, rebase, or workspace behavior, prefer fixture-backed before/after
  snapshots using existing visualizers, plus targeted structural assertions for
  the invariant.
- After changing Rust APIs or types exposed through `@gitbutler/but-sdk`, run
  `pnpm build:sdk && pnpm format`; this updates
  `packages/but-sdk/src/generated`.

## Assertions

- Explain why a standard Rust assertion holds concisely using the last-argument message, like `assert!(1!=2, "arithmetic unit on CPU works")`
- Snapshot assertions use `snapbox`, like `snapbox::assert_data_eq!(actual, snapbox::str![[r#"..."#]])`. `snapbox::assert_data_eq!` has no message argument, so explain why a snapshot holds with a `// comment` on the line above it.
- Regenerate inline snapshots by running the tests with `SNAPSHOTS=overwrite`.
- Remove unstable output before asserting (sanitize ids/timestamps/paths, e.g. the `but_testsupport` helpers) rather than snapshotting it raw. `snapbox` also pattern-matches expected data by default (`[..]` and `...` wildcards, path-separator normalization); append `.raw()` to the expected `snapbox::str!` when the value must match exactly (e.g. it contains backslashes or literal `[..]`/`...`).
