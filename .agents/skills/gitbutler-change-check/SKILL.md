---
name: gitbutler-change-check
description: "Use when developing or modifying GitButler code, fixing bugs or regressions, addressing PR feedback, or adding or changing tests and fixtures. Read applicable AGENTS.md files before editing, choose the repository's test harness, and validate the smallest behavior change. Applies to Rust, CLI/TUI, Svelte, React, and TypeScript work."
---

# GitButler Change Check

## Before Editing

1. Identify the requested behavior and the files likely to change. Keep questions
   and review-only requests read-only unless changes are authorized.
2. Read [the root instructions](../../../AGENTS.md) and every applicable ancestor
   `AGENTS.md`, including nested instructions for the implementation, tests,
   fixtures, and documentation. Apply their precedence rules. Repeat this step
   when scope expands; an earlier read of a neighboring file's instructions is
   not sufficient. Existing code is evidence of a pattern, not permission to
   ignore current instructions.
3. Load the relevant domain guidance:
   - Rust: [crates/AGENTS.md](../../../crates/AGENTS.md). For graph, workspace,
     dependency, reachability, ordering, or history operations, also read
     [WORKSPACE_MODEL.md](../../../crates/WORKSPACE_MODEL.md).
   - CLI: [crates/but/AGENTS.md](../../../crates/but/AGENTS.md) and the
     `cli-commands` skill; load `tui-tests` or `but-performance-tests` when their
     test surfaces are involved.
   - Frontend: applicable app/package instructions and
     [frontend.md](../../../frontend.md) for tests. Load the relevant UI,
     render-performance, or screenshot skills; do not apply one frontend's
     framework or import rules to another.
4. Read the nearest owning implementation and a representative test. State one
   local hypothesis and the cheapest check that could disprove it. For a behavior
   bug, reproduce it before adding machinery. For a test-only migration, preserve
   the original regression's assertions and run that same test immediately.

## Choose The Test Harness First

- For Rust repository, graph, rebase, and workspace tests, put reusable history
  setup in the owning crate's `tests/fixtures/scenario/` scripts. Reuse or extend
  a suitable scenario before adding one. Use `but_testsupport::writable_scenario`
  for mutations or the existing CLI `Sandbox` helpers. Do not construct a Git
  repository in a raw `tempfile` merely because a neighboring test does so.
- Use the existing read-only fixture helper for read-only behavior. Do not mutate
  cached shared fixtures. Keep writable fixture guards alive for the whole test.
  Follow nearby fixture conventions for isolated configuration, frozen identities
  and dates, refs, and initial worktree state.
- Keep the test focused on the operation and its before/after invariants. Use
  existing visualizers and snapbox snapshots where instructed, supplemented by
  precise assertions for content, refs, rejection counts, and preserved changes.
  Explain assertions as required by the scoped instructions.
- In CLI tests, use `env.but(...).assert().success()/failure()` with stdout/stderr
  snapshots and the existing Git/shell sandbox helpers. Do not weaken snapshots
  into substring checks or blanket-overwrite expected output to make a test pass.
- Scale coverage to the risk: pair the reported failure with the nearest behavior
  that must remain valid. For example, boundary matching needs a nonmatching
  neighbor; identity/counting fixes need multiple hunks or rewritten commits;
  ancestry fixes need same-branch descendants and cross-branch ancestors.

## Implement And Verify

1. Make the smallest change supported by the regression. Preserve API and
   permission boundaries, transactions, rollback/undo, dry runs, byte-preserving
   paths, and graph semantics as required by the Rust instructions. Propose new
   mechanisms before building them.
2. Immediately run the focused test or check. Fix a local failure before widening
   scope. Then run required scoped formatting, linting, and adjacent contract
   checks. Do not format unrelated dirty files or assume `pnpm isgood` runs tests.
3. For shared behavior, identify affected callers and transport contracts; update
   them or explain why they are unchanged. Run SDK regeneration when required by
   the Rust instructions, not for unrelated internal changes.
4. Before declaring the work ready or publishing it, load
   [gitbutler-review-prepush](../gitbutler-review-prepush/SKILL.md). Report exactly
   what ran and what remains unverified; tests reduce risk, not prove its absence.
