---
name: gitbutler-review-prepush
description: "Use when reviewing GitButler code or pull requests, addressing review findings, checking regressions or convention compliance, or preparing to commit, push, publish, update a PR, or ship it. Re-read applicable AGENTS.md files, audit test fixtures and behavior, and check validation evidence before publishing. This skill does not itself authorize code changes or a push."
---

# GitButler Review And Pre-Push Check

## Establish Scope And Rules

1. Identify the exact branch, commits, or dirty files under review. In a workspace
   with multiple applied branches, do not treat every worktree change as part of
   this PR. Preserve unrelated work and review the full proposed PR diff, not just
   its last fix. Read current review threads when addressing PR feedback.
2. Read [AGENTS.md](../../../AGENTS.md) and all applicable ancestor and nested
   `AGENTS.md` files for every changed area, including tests, fixtures, and docs.
   Do this for reviews and pre-push checks even if implementation is finished.
   Resolve the instructions against the actual changed paths, not memory alone.
3. For Rust, read [crates/AGENTS.md](../../../crates/AGENTS.md); for graph or
   workspace relationships, also read
   [WORKSPACE_MODEL.md](../../../crates/WORKSPACE_MODEL.md). Apply CLI, app,
   package, frontend-test, and domain-skill guidance where the diff requires it.
   Do not duplicate those manuals or infer conventions from old violations.

## Audit Before Publishing

- Check the requested behavior and its closest counterexample. Look for changed
  count units, coordinate spaces, ordering, commit identity after rewrites,
  ancestry versus branch labels, and cross-stack ambiguity when relevant.
- Check the Rust contracts required by its instructions: ownership boundaries,
  permission reuse, graph semantics, transaction rollback, undo, dry-run effects,
  byte-preserving data, error context, and database compatibility. For frontend
  changes, apply the owning framework's state/render and design conventions.
- Audit new tests independently of whether they pass. Repository history for
  graph/workspace behavior belongs in `but_testsupport` scenarios or the CLI
  `Sandbox`, not manual `git init` in a raw `tempfile`. Check deterministic setup,
  correct read-only/writable fixture choice, and assertions that still reproduce
  the claimed regression. Isolated Git command helpers alone are not a substitute
  for the fixture harness.
- Check that snapshots show the intended result, normalize unstable data, and
  retain useful diagnostics. Verify failure cases preserve the relevant worktree
  and refs, and successful cases change only the intended content.
- Verify each affected shared caller, SDK/transport contract, and documentation
  surface was updated or explicitly found unaffected. Use scoped instructions to
  determine required formatting, lint, test, feature, and generated-output gates.
- Distinguish passing local commands from ignored tests, checks not run, and live
  CI results. Do not claim a full suite passed when a filter ran no tests. A green
  build or resolved review thread does not establish convention compliance or
  guarantee that regressions are impossible.

## Act On Findings

1. For a review-only request, present actionable findings first with locations,
   impact, and missing tests; do not silently edit or publish. If none remain,
   state that and disclose verification gaps.
2. For authorized fixes, use
   [gitbutler-change-check](../gitbutler-change-check/SKILL.md), validate the
   correction, and recheck the resulting diff against the scoped instructions.
   If a required gate cannot run, report the blocker rather than calling it passed.
3. Commit or push only when authorized. Use the existing GitButler version-control
   skill, select only the intended changes, and preserve unrelated work. Keep
   commit messages and PR descriptions within repository conventions.
4. After a successful push, explain addressed review findings with the actual
   changes and validation, resolve only addressed threads, and keep the PR
   description accurate. Report remaining CI failures or authorization blockers;
   do not hide them or mark unrelated findings resolved.
