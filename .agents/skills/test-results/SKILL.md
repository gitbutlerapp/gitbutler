---
name: test-results
description: Query CI test results, failures, and flake evidence from tests.but.dev. Use after pushing a branch to check whether CI failures are yours or known-flaky, when investigating a failing or flaky test, or to pick which flaky test to fix.
---

All six CI suites (cargo, vitest, playwright, playwright-ct, lite-e2e,
blackbox) report every test result — including full failure output — to
`https://tests.but.dev`. The API is unauthenticated JSON over GET; it answers
questions that otherwise require downloading GitHub Actions logs. Data exists
from 2026-08-28 onward.

## Check CI on a branch ("are these failures mine?")

One call returns every suite's latest run on your branch with failures
inlined:

```console
$ curl -s "https://tests.but.dev/api/report/branch?branch=<branch>"
```

Per suite: `run` (counts, commit, CI link) and `failures[]`, each with
`failureReason` (one line), `failureExpanded` (stack/snippet), and
`knownFlaky`. Interpretation:

- `knownFlaky: true` — the flake detector already suspects this test from
  prior evidence; the failure is probably not your change. Retrying is
  reasonable; don't chase it.
- `knownFlaky: false` — treat as caused by your branch. The expanded output
  is usually enough to fix without opening CI logs.
- A suite absent from the report has no runs on that branch (path filters
  skip jobs; blackbox never runs on master).

URL-encode the branch (it may contain `/`). Uploads land when a suite's job
finishes, not when the workflow ends — poll per suite. Verify a specific
pushed commit instead with `curl -s https://tests.but.dev/api/commits/<sha>`.

## Investigate or fix a flaky test

Pick a target from the evidence-ranked leaderboard:

```console
$ curl -s "https://tests.but.dev/api/flaky?days=14"
```

Ranked by `retryFlakeRuns` (fail→pass across attempts inside one run — the
code didn't change, so this is proof, not correlation), then
`disagreeingCommits`, then default-branch fail rate. Prefer targets with
`retryFlakeRuns > 0`. Then fetch everything about one test:

```console
$ curl -s "https://tests.but.dev/api/report/test/<testId>"
```

Returns identity (`scope`, `name`, `fileName`), `evidence` (including the
actual disagreeing commit shas — useful for bisecting), `failureClusters`
(reasons grouped with volatile numbers/hashes normalized away; 37 failures
collapsing into one cluster means one root cause), `latestFailureExpanded`
(full stack), and `recentRuns` with per-attempt outcomes.

Reproduce locally by retrying: `cargo nextest run -E 'test(<name>)'
--retries 20` for cargo, `--repeat-each=20` for playwright suites. After a
fix merges, success = the test's `recentRuns` stay green; the timeline is the
metric, not a single pass.

## Other useful queries

- `GET /api/tests?q=<substring>[&suite=<name>]` — find a test's id by name.
- `GET /api/runs` — recent runs across suites off master (the Branches feed).
- `GET /api/suites?days=7` — per-suite health (append `&allBranches=true` to
  include PR runs).

Failure output is capped (reason 4KB, expanded 32KB) and videos/artifacts are
not stored — for those, follow the `url` field to the GitHub Actions run.
