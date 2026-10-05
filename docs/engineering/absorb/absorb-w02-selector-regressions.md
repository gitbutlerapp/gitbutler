# W02: Reproduce Absorb Selection Failures

Status: brief prepared; execution requires completed W00 evidence.
Scope: fixture-backed regression tests and evidence only. No production fixes.

## Assignment For The Agent

Complete W02 only. Establish whether coupled selections and multiple old-side
selections retain their meaning through absorb. Leave named executable tests and
observed outcomes in [the ledger](absorb-verification-ledger.md).

Read current root and applicable scoped instructions, required change/test skills,
the plan's Approved Product Policy and Execution Protocol, and W00 evidence.
Use W01 findings if available; W01 is useful but is not a prerequisite for W02.
Do not infer technical design approval from product-policy approval.

This brief refines [W02 in the plan](absorb-implementation-plan.md#w02-reproduce-selection-failures).
The ledger owns live state. Preparing this brief does not execute the packet.

## Permissions And Boundaries

- Edit existing absorb API test sections and owning-crate scenario fixtures,
  plus the ledger. Prefer existing test homes and helpers over additional files.
- Do not change production selection, routing, transaction, or amendment logic.
  Do not expose private production helpers merely to make testing convenient.
- No ignored tests, expected-panic disguises, weakened assertions, blanket snapshot
  regeneration, dependency changes, commits, pushes, branch changes, or PR updates.
- New tests may intentionally fail until W05. Report that state explicitly; do
  not claim release readiness or repair the implementation in this packet.
- Do not run absorb against the development repository. All mutations belong in
  writable fixture copies whose guards remain alive through all assertions.

## Step 1: Claim And Verify The Test Boundary

Verify usable W00 evidence and current source identity before claiming W02. Check
ownership and relevant dirty changes; preserve others' edits. If W00 is missing,
stop and name the prerequisite rather than executing it silently.

Start with these existing entry points, confirming their current signatures:

- [Absorb API tests and executor](../../../crates/but-api/src/legacy/absorb.rs).
- [Existing rejection scenario](../../../crates/but-api/tests/fixtures/scenario/absorb-rejected-hunks.sh).
- [Selection interpretation contract](../../../crates/but-core/src/tree/mod.rs).
- [Existing selector tests](../../../crates/but-core/src/tree/tests.rs).
- [Absorb input types and conversion](../../../crates/but-hunk-assignment/src/lib.rs).

Use public operation boundaries or the existing permission-taking absorb API.
A hand-constructed plan isolates executor behavior; a separate planner-plus-apply
case is needed to test routing. A scheduler sort assertion alone is insufficient.
Do not fabricate harness methods from these descriptions: inspect nearby tests.

Before editing, state the local hypothesis and the exact observable that would
disconfirm it. For example: splitting paired selections changes the committed
blob; an exact blob comparison after absorption can demonstrate or refute that.

## Step 2: Build The Smallest Deterministic Fixture

Use `but_testsupport::writable_scenario` with an owning-crate scenario script and
the existing isolated configuration conventions. Reuse suitable setup or add one
focused fixture when existing scenarios cannot express the cases clearly.

For the first executor case, create an empty base and one mutable target commit
containing ten distinct newline-terminated lines `old-01` through `old-10`.
Use a dirty worktree containing `new-01` through `new-10`. No lines are identical
between images, so a zero-context diff should contain one ten-line replacement.
Verify that actual diff shape as a fixture precondition rather than assuming it.
Configure target metadata and context through established test helpers/patterns.

Expected blobs must come from explicit literal content or simple independent
line-list construction. Do not calculate expected content using absorb's scheduler,
selector conversion, or patch-application code under test.

## Step 3: Add And Run Cases Incrementally

The case IDs below identify evidence, not existing Rust test names. Choose clear
names consistent with nearby tests and record the mapping in the ledger.
Tuple notation is `(old_start, old_lines, new_start, new_lines)`.

### S1: Paired Replacement In One Underlying Hunk

Use the ten-line fixture and select, in this order:

```text
(5, 1, 0, 0)
(0, 0, 5, 1)
```

Expected target blob: `old-01` through `old-04`, then `new-05`, then `old-06`
through `old-10`, each newline-terminated. Expected worktree remains all ten
`new-*` lines. Valid selections should not be rejected. Assert the exact blob,
unchanged worktree, and independently checked remaining uncommitted changes.

Do not sort the input to accommodate the implementation. Relative selection
order is part of the existing interpretation contract. Record wrong content,
unexpected rejection, or an error as distinct failure modes.

### S2: Multiple Old-Only Selections

Start from a fresh copy of the same fixture, not S1's amended state. Select:

```text
(2, 1, 0, 0)
(7, 1, 0, 0)
```

Expected target blob contains the original lines numbered 1, 3, 4, 5, 6, 8, 9,
10, in order. No `new-*` line is committed. The worktree remains all ten new
lines. This distinguishes deleting original selected lines from deleting shifted
neighbors after the first amendment. Both selectors have the null new-side
sentinel; it must not be treated as a meaningful file position for ordering.

### S3: Mixed Full And Partial Selections

Extend or reuse a fixture with a second changed region separated by enough unique
unchanged lines to stay independent at the chosen context setting. Select the
first region as a full hunk and a paired subset of the second region. Leave at
least one change unselected. Obtain full-hunk headers from the verified diff;
do not guess offsets after inserting fixture padding.

Assert exact target content and residual changes. Keep it to one target initially
to isolate selection semantics. Add a default-context counterpart when it tests
a meaningful grouping difference; avoid a full Cartesian test matrix in W02.

### S4: Planner Plus Executor Routing

Pass the selected hunks through `absorption_plan_with_perm` (or the current public
equivalent), then apply that plan. First use an unambiguous single-target history
so unexpected routing cannot be dismissed as legitimate ambiguity.

Next construct a nearby two-commit case in which coupled selectors might receive
different dependency matches. Inspect and record the proposed destinations before
execution; assert exact per-commit content when the destination is justified.
If the case is genuinely ambiguous, the approved policy requires rejection, not
guessing a destination. Record it as a routing/atomicity input for W03/W04 rather
than inventing an expected target. Do not demand a particular internal group
count if several representations preserve the same selection and routing meaning.

## Step 4: Validate Without Fixing The Defect

After each substantive test edit, immediately run the new named test. Start from:

```sh
cargo test -p but-api legacy::absorb::tests -- --list
cargo test -p but-api <actual_new_test_name> -- --nocapture
```

Replace the placeholder with the real name; verify a nonzero selected test count.
Compilation/fixture failures are not reproduced absorb defects. Repair only test
setup mistakes within this packet and rerun the same focused command. If expected
semantics remain unclear, consult the existing selection contract and record a
design question instead of making the expectation match the observed output.

After adding the cases, run the absorb API test group and, when needed to check
the underlying interpretation, `cargo test -p but-core to_additive_hunks`.
Compare existing-test results with W00. The group may legitimately be red because
of new regressions; report old and new results separately. Do not demand every
new case fail: passing counterexamples constrain the eventual fix too.

Validate scoped Rust formatting and fixture shell syntax with existing repository
commands. Do not run snapshot-overwrite mode on tests whose expected result is
the correctness specification. Preserve strong existing assertions unless a
later authorized policy-change packet explicitly replaces them.

## Step 5: Evidence And Handoff

For every case record test name, fixture, exact source identity, invocation and
count, expected versus actual content/outcome, and classification:
confirmed regression, passing counterexample, fixture/setup issue, or unresolved
semantic question. State whether the planner or executor boundary was exercised.

Document how residual changes were checked; worktree preservation alone does not
prove correct history, and a zero rejection count alone does not prove selection
fidelity. Inspect relevant refs and descendant content in multi-commit cases,
without asserting that legitimate rewritten commit IDs remain unchanged.

W02 is done when S1/S2 have named executable evidence (or a source-backed narrowed
finding), mixed-selection and planner coverage are recorded, and remaining gaps
have a precise owner/next action. Unsupported claims must be narrowed or withdrawn,
not kept as confirmed failures. Unresolved essential semantics block completion.

Update the ledger only after reviewing the test diff and validating the documents.
Clear ownership and set the next ready packet: W01 if its audit is still pending,
otherwise W03. Do not mark F1/F2 fixed, approve W04, or start production work.
The final handoff must explicitly say whether the workspace now contains expected
failing tests and list their names. Stop after W02.

## Prompt To Assign This Packet

> Execute absorb-w02-selector-regressions.md only after verifying W00 evidence.
> Add fixture-backed tests for paired, old-only, mixed, and planner-routed
> selections, running each immediately and comparing exact expected content.
> Record failures and passing counterexamples in absorb-verification-ledger.md.
> Do not fix production code, ignore failing tests, commit, push, switch branches,
> or update GitHub. Stop with named test evidence or a precise blocker.
