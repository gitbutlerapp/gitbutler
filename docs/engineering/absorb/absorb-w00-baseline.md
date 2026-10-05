# W00: Establish The Absorb Baseline

Status: ready to execute, not executed. Prerequisites: none.
Scope: source identification, existing-test discovery, and baseline evidence.
Do not implement new tests or production changes in this packet.

## Assignment For The Agent

Complete W00 only. Your deliverable is a source-backed baseline entry in
[the ledger](absorb-verification-ledger.md), sufficient for W01 and W02 to start
without repeating repository discovery. Do not proceed into either packet.

Read current root and applicable scoped instructions first. In
[the plan](absorb-implementation-plan.md), read Approved Product Policy,
Execution Protocol, and W00. Read the ledger's Resume Here and approved decisions.
Use the GitButler skill before version-control inspection and Rust/CLI scoped
instructions before running their tests. Read repository memory when available,
but verify its claims against the current environment.

This brief refines W00; it does not override the plan or repository instructions.
The ledger alone owns live task state and results. Do not tick the checklist here
as well: report these acceptance items in the ledger to avoid divergent copies.

## Permissions And Boundaries

- Manual edits: ledger only. Build/test artifacts are expected; inspect unexpected
  tracked-file changes and preserve them rather than reverting automatically.
- No production changes, new fixtures, snapshot regeneration, dependency updates,
  formatting unrelated files, commits, pushes, branch changes, or PR modifications.
- No `but setup`, clean/reset/stash, or destructive recovery to simplify the state.
- No baseline absorb commands against this development repository. Run behavior
  checks through fixture-backed tests, which create their own repositories.
- Do not install missing tools or alter toolchains as an incidental workaround.
  Record the prerequisite and ask when environmental changes are necessary.

## Step 1: Claim And Identify

Read the ledger before claiming W00. If another owner is active, or newer evidence
already completes it, do not overwrite their state. Resolve ownership or continue
only with explicit direction. Otherwise mark W00 active and record date/owner.

Use read-only commands from the workspace root to establish actual state:

```sh
pwd
git rev-parse --show-toplevel
git rev-parse HEAD
git status --short
git rev-parse --verify fix-absorb-deletion-boundary
git merge-base master fix-absorb-deletion-boundary
target/debug/but status
```

Verify names/tools before relying on them. Historical context suggests the local
development `target/debug/but` understands this workspace while installed `but`
may not. If the development binary is absent, record it; do not run setup or
build a replacement merely to obtain status. Read-only Git can identify commits
but may not fully explain GitButler's applied-branch composition.

Record full immutable IDs, not just `HEAD`, `master`, or relative `~N` names.
Distinguish the absorb branch tip from the checkout/workspace commit and from
unrelated applied branches. Local `master` is not necessarily current upstream.
The historical reviewed absorb head was
`64436487e7c57bdfde3da4785f1e9d6e52c38d50`; mismatch is a fact to explain, not
permission to reset. PR #16181 is historical context; #16182 was attached later
without an established relationship. Verify any remote comparison read-only and
record its source; do not infer scope from the currently displayed PR.

Inspect only relevant dirty diffs using the required GitButler workflow. Record
paths and how they affect attribution; do not copy unrelated contents or secrets.
If the intended implementation cannot be distinguished, mark blocked and stop.

## Step 2: Locate Existing Tests And Instructions

Verify these entry points still exist; follow a rename locally if necessary:

- [Absorb API and unit tests](../../../crates/but-api/src/legacy/absorb.rs)
- [Absorb rejection fixture](../../../crates/but-api/tests/fixtures/scenario/absorb-rejected-hunks.sh)
- [Selector contract tests](../../../crates/but-core/src/tree/tests.rs)
- [CLI absorb tests](../../../crates/but/tests/but/command/absorb.rs)
- [CLI ordering fixture](../../../crates/but/tests/fixtures/scenario/absorb-parent-before-child.sh)

Read applicable ancestor/nested AGENTS.md files, not just these entry points.
Record test names, owning crate/target, fixture helper, and what each protects.
At minimum locate coverage for rejection counting, per-path ordering, ambiguous
locks, deletion-boundary matching, CLI ancestor/descendant absorption, and partial
selection interpretation. Do not map unrelated suites.

Use listing to verify that filters actually select the expected tests:

```sh
cargo test -p but-api legacy::absorb::tests -- --list
cargo test -p but --test but command::absorb -- --list
```

These commands may compile. A compile failure is not a test failure; record the
distinction. Current counts may differ from historical counts. Zero selected
tests is never success: correct a renamed filter or report the missing coverage.

## Step 3: Run The Focused Baseline

Run serially to keep outcomes attributable and avoid Cargo build-lock contention:

```sh
cargo test -p but-api legacy::absorb::tests
cargo test -p but --test but command::absorb
```

For each command record exit status, test target, passed/failed/ignored counts,
and the names and concise failure output for failed tests. Record the source
identity from Step 1 and any relevant dirty state. Historical results were four
API tests and fifteen CLI tests; these are reference values, not assertions.

Do not rerun full suites, lint, benchmarks, or SDK generation merely for baseline
reassurance. W00 is not release validation. If a narrow command fails, inspect
the immediate error once to classify it as tooling/build, fixture/environment,
test assertion, or unknown. Do not repair it in this packet or loop blindly.

A failing baseline may still be a complete characterization if its cause and
impact are bounded. An unknown failure preventing useful next-step evidence is
a blocker. Passing on a combined dirty workspace must be labeled as such, not
claimed as a clean test of the published PR head. Request permission for an
isolated setup only when contamination makes the next work unsafe or inconclusive.

## Step 4: Leave A Test Roadmap, Not New Tests

Record the concrete existing harness/fixture to extend and the cheapest named
test command for each future case. These belong to W02/W03, not W00:

| Future case                                                | Expected assertion                                              | Owner |
| ---------------------------------------------------------- | --------------------------------------------------------------- | ----- |
| Paired old/new line-5 selections in a ten-line replacement | Only selected line replaced in target blob; worktree preserved  | W02   |
| Several old-only selections                                | Original selected lines removed, not shifted neighbors          | W02   |
| Mixed full/partial selections and planner routing          | Exact committed/residual content; coupled meaning retained      | W02   |
| Late rejection after an earlier applicable group           | Entire selected operation unpublished; relevant state unchanged | W03   |
| Cross-branch rejection or planning-created blank commit    | No successful subset or planning ref escapes                    | W03   |
| Selected landed/immutable target                           | Explicit failure, not skipped input reported as success         | W03   |

Use existing scenario helpers and tests as anchors; do not prescribe fabricated
helper APIs or test names as if they already exist. Record proposed names as
proposed. The existing partial-success rejection test reflects the old policy;
do not alter it or label it sufficient evidence for atomicity.

## Step 5: Verify And Hand Off

Recheck source/dirty state after the commands. If the source changed during the
run, explain affected evidence and rerun only what is necessary, or mark it
inconclusive. Do not attribute another agent's edits to test generation without
evidence.

Append one W00 evidence entry using the ledger's handoff template. It must include:

- Exact branch/workspace/base IDs and observed applied/dirty state.
- Applicable instructions and test entry points verified.
- Commands and nonzero test counts, including failures or unrun checks honestly.
- Existing coverage map and proposed W02/W03 test homes.
- Attribution/isolation limitations and next action.

Validate ledger Markdown with the existing formatter, scoped to that file. Do
not add a new formatting dependency. If the formatter is unavailable, disclose it.
Mark W00 done only when source identity and baseline evidence are usable; otherwise
mark blocked with the specific missing prerequisite. On completion set Resume Here
to W01, clear the active owner, and state its first action: audit planner side
effects and transaction coverage. Do not mark F1/F2 fixed or a phase approved.

Final response: baseline identity, test results, limitations, ledger location, and
next packet. No implementation claims. Stop after W00.

## Prompt To Assign This Packet

> Execute absorb-w00-baseline.md only. Read its required instructions and claim
> W00 in absorb-verification-ledger.md. Identify the actual source and local
> changes, discover and run the existing focused tests, and record an actionable
> baseline plus test roadmap. Do not implement fixes or new tests, commit, push,
> switch branches, or update GitHub. Stop with evidence or a precise blocker.
