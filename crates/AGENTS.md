# Rust Instructions

## API Boundaries and Legacy

- Treat `gitbutler-*` crates as legacy-heavy, but preserve local ownership and
  nearby patterns for localized fixes. Do not introduce new `gitbutler-*` usage
  in newer code unless the surrounding legacy boundary requires it. Keep legacy
  fixes localized; migrate only when necessary, tiny, tested, and
  behavior-neutral.
- Minimize new `VirtualBranchesHandle` usage. At API boundaries, prefer
  `but_ctx::Context` workspace helpers and newer `but-*` APIs.
- `but-api` is the API surface for Tauri, CLI, and TUI callers.
  Outer callers should prefer existing `but-api` functions when they fit, but
  lower-level crates should not depend on `but-api`.
- Preserve existing `but-api` macro, transport, serialization, and conversion
  patterns instead of hand-rolling parallel wrappers.
- Keep transport DTOs at the API boundary, commonly in a local `json` module,
  and convert them to domain types before calling lower-level crates.
- For permissioned `but-api` functions, follow the existing composition shape:
  acquire permission near the wrapper and delegate to a `_with_perm` or other
  permission-taking implementation.
- API consumers such as the CLI, TUI, and other composed callers should use
  `_with_perm` variants when they already hold permission to avoid extra locking
  and deadlock risk.
- Keep `Context` at API/composition boundaries; pass granular dependencies
  like repo, workspace, metadata, database handles, or an `Editor` into
  lower-level crates where possible.
- Acquire repository access locks at top-level API/command boundaries. Do not
  call helpers that acquire permissions while holding a guard; drop the guard or
  call a permission-taking helper.
- Preserve existing `DryRun` semantics; add dry-run support when the API/product
  surface calls for preview behavior. Dry runs should avoid persisting refs,
  objects, or oplog entries.
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
  graph/history/ref-placement mutations, use `crates/WORKSPACE_MODEL.md` as the
  reference. In short: prefer commit IDs and refs at API boundaries, convert to
  operation-local selectors inside editor-backed operations, use
  `but_graph::Graph` for relationship/reachability questions, use
  `but_rebase::graph_rebase::Editor` for Git graph/history/ref rewrites where an
  editor model exists, and treat `but_graph::Workspace` and
  `but_workspace::RefInfo` as lossy presentation/compatibility views unless an
  existing workspace-shaped boundary requires them.

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
- Use existing reload/invalidation helpers after external repo or worktree
  changes, especially when working through `Context` caches.
- When frontend/API consumers need to classify an error, use existing
  `but_error::Code` patterns; do not make consumers match error strings.

## Database Migrations (`but-db`)

- Keep migrations forward-compatible: stay on the current `SchemaVersion` and
  leave columns or tables that new code stopped using in place. Bumping the
  version locks every older binary out of the database; reserve it for planned,
  coordinated breaks, never routine cleanup.

## Version Control

- When asked to branch, commit, push, or open a PR, use the GitButler `but`
  CLI/workflow when available.
- When the user says "ship it", commit your changes on a session branch,
  creating one if needed, then push and open or update the PR. Reuse the
  existing branch/PR when it already fits the session.
- Do not list local validation commands in commit messages or add AI co-author
  trailers or tool branding.

## Testing and Validation

- Use existing test-support crates and fixtures; prefer read-only fixtures for
  read-only behavior.
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
