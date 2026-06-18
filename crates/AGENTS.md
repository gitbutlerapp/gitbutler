# Rust Instructions

These are defaults for Rust source and test changes under `crates/`; more
specific instruction files override them. Do not apply them to generated,
vendored, or fixture data unless the task is specifically about that code.

## API Boundaries and Legacy

- Treat `gitbutler-*` crates as legacy-heavy, but preserve local ownership and
  nearby patterns for localized fixes. Do not introduce new `gitbutler-*` usage
  in newer code unless the surrounding legacy boundary requires it. Keep legacy
  fixes localized; migrate only when necessary, tiny, tested, and
  behavior-neutral.
- Minimize new `VirtualBranchesHandle` usage. At API boundaries, prefer
  `but_ctx::Context` workspace helpers and newer `but-*` APIs.
- `but-api` is the API surface for Tauri, Electron/N-API, CLI, and TUI callers.
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

## Code Shape and Naming

- Use Git/GitButler domain names and keep types/helpers in the crate or module
  that owns the concept.
- Avoid vague names like `Manager`, `Service`, `Helper`, `Util`, or `Processor`
  unless nearby code uses that concept.
- Solve the present problem directly. Avoid speculative abstractions: one-use
  traits/types/functions, fake extension points, ceremonial wrappers/modules,
  and public APIs larger than real callers need.
- Keep module boundaries discoverable from names and the call graph.
- Avoid drive-by refactors while fixing a specific bug.
- Prefer explicit domain enum matches when wildcard arms would hide behavior.
- Comment public API purpose/invariants/errors and complex algorithm intent;
  avoid comments that merely repeat the code.

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
- Treat filesystem and worktree writes carefully; use existing locking,
  transaction, and access patterns.
- Avoid implicit `SystemTime::now()` in testable business logic; pass time in
  when deterministic behavior matters.
- Use existing reload/invalidation helpers after external repo or worktree
  changes, especially when working through `Context` caches.
- Use `anyhow::Context` to explain what operation failed. When frontend/API
  consumers need classification, use existing `but_error::Code` patterns; do
  not make consumers match error strings.

## Version Control

- Assume the worktree may contain other agents' changes. Do not overwrite, clean
  up, stage, commit, or amend changes you did not make.
- When asked to branch, commit, push, or open a PR, use the GitButler `but`
  CLI/workflow when available.
- When the user says "ship it", commit your changes on a session branch,
  creating one if needed, then push and open or update the PR. Reuse the
  existing branch/PR when it already fits the session.
- For small cleanup or follow-up fixes on your own branch, amend the relevant
  existing commit(s) when that is the cleaner history.
- Keep commit messages and PR descriptions succinct: why, impact, and core
  decisions. Do not list local validation commands in commit messages or add AI
  co-author trailers or tool branding.

## Testing and Validation

- Run the narrowest relevant test or check first, for example `cargo test
  -p <crate> <test-name>` or `cargo check -p <crate> --all-targets`.
- Use existing test-support crates and fixtures; prefer read-only fixtures for
  read-only behavior.
- For graph, rebase, or workspace behavior, prefer fixture-backed before/after
  snapshots using existing visualizers, plus targeted structural assertions for
  the invariant.
- When snapshot output is volatile, stabilize inputs or normalize output; do not
  replace strong snapshots with vague assertions.
- When optimizing Git traversal or workspace behavior, preserve semantics with a
  fixture-backed regression test.
- Run `cargo fmt` for Rust formatting, avoiding unrelated dirty files.
- Run `cargo clippy --fix --allow-dirty` only after inspecting the diff scope and
  only when generated edits stay within the intended change.
- Run `cargo machete` when dependencies changed; verify targets, features, and
  platform-specific usage before removing dependencies.
- After changing Rust APIs or types exposed through `@gitbutler/but-sdk`, run
  `pnpm build:sdk && pnpm format`; this updates
  `packages/but-sdk/src/generated`.
- Use workspace-wide checks only when the change affects shared contracts or
  multiple crates.

## Assertions

- Explain why a standard Rust assertion holds concisely using the last-argument message, like `assert!(1!=2, "arithmetic unit on CPU works")`
- Explain why an `insta` assertion holds concisely using the second argument as message, like `insta::assert_debug_snapshot(debug, "needs to be this because...", @r"")`.
- Use `insta` redactions to remove unstable output from the snapshot. Avoid *creating* additional macros, as it's possible to change its settings
  directly. It's fine to use its own utility macros to configure redactions, where applicable.
