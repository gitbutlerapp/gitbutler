# W03: Specify Absorb Atomicity In Executable Tests

Status: brief prepared; execution requires completed W01 evidence.
Scope: fixture-backed tests, narrowly necessary test helpers, and evidence only.

## Assignment For The Agent

Complete [W03 in the plan](absorb-implementation-plan.md#w03-specify-atomicity-in-executable-tests).
Encode the approved all-or-nothing invocation policy in executable tests. This
includes planning and independent branches, not merely one amendment call.
Record results in [the ledger](absorb-verification-ledger.md).

Read current applicable instructions and required change/test skills before
editing tests. Use W01's side-effect inventory, caller paths, and proposed test
homes. W02 evidence is useful but not a formal prerequisite. Do not conflate
selector corruption with the pre-existing partial-application policy.

Preparing this brief does not execute W03. Technical design still needs W04
approval; product-policy approval does not authorize production implementation.

## Permissions And Boundaries

- Edit fixture-backed API/CLI tests, owning-crate scenario fixtures, and the ledger.
  Add a local test helper only if existing helpers cannot express the comparison.
- Keep production code, public APIs, generated bindings, and dependencies unchanged.
  A new production fault-injection mechanism requires a later approved design.
- Use `but_testsupport::writable_scenario` and retain its guard for API fixtures;
  use established CLI `Sandbox` patterns where the CLI controls the behavior.
- Never run absorb against the development repository. Do not commit, push,
  switch branches, update GitHub, or start another packet.
- Preserve existing tests. Do not ignore new failures, mark them expected panics,
  regenerate snapshots to match incorrect results, or weaken the policy to pass.

## Step 1: Claim The Packet And Choose The Boundary

Verify that W01 is complete with source-backed state and caller tables. Confirm
current source identity and relevant dirty edits, then claim W03 in the ledger.
If the audit is missing, stop with that prerequisite; do not silently perform W01.

Start at W01's identified owning boundaries, typically:

- [Absorb planning and execution](crates/but-api/src/legacy/absorb.rs).
- [CLI orchestration](crates/but/src/command/legacy/absorb.rs).
- [Existing CLI absorb tests](crates/but/tests/but/command/absorb.rs).
- [Transaction behavior](crates/but-transaction/src/lib.rs).

State one testable hypothesis before the first edit: an earlier applicable change
is published despite a later rejection. A before/after ref and blob comparison
can disconfirm it. Use simple independent full-hunk selections where possible so
W02's coupled-selector defect does not obscure this policy test.

Exercise the real invocation boundary, including planning, eligibility filtering,
and wrapper side effects. A hand-constructed executor plan can isolate the late
rejection, but cannot alone establish invocation atomicity. If callers split
planning and application, snapshot before the first request and run both phases.
Record both the tested boundary and any uncovered wrapper or transport behavior.

## Step 2: Define The State Oracle

Capture semantic state before planning and compare it after the operation returns.
Derive the exact capture APIs from W01 and neighboring tests; do not invent helper
methods. Prefer structured values over snapshots of complete directories.

| State         | Required comparison                                                                                         |
| ------------- | ----------------------------------------------------------------------------------------------------------- |
| Worktree      | Exact relevant tracked/untracked bytes, existence, modes, and symlink targets where applicable              |
| Index         | Paths, stages, modes, object IDs, and other meaningful state identified by W01                              |
| Refs and HEAD | Relevant branch/workspace refs, symbolic HEAD or detached identity, and new/deleted refs                    |
| History       | Target and descendant blobs plus reachable graph relationships                                              |
| Metadata      | Relevant workspace/stack/branch records, including creation or removal                                      |
| Database      | Relevant rows and assignments identified by the audit, not raw database/WAL bytes                           |
| Oplog         | User-visible operation entries and head, with any existing pre-operation snapshot side effect made explicit |

Use stable ordering for comparisons; do not normalize away meaningful differences.
File mtimes, cache files, and unreachable Git objects are not primary policy
oracles. New reachable blank commits, moved refs, changed assignments, or visible
oplog entries are meaningful. State any exclusions and why they are not visible
operation effects. Existing side effects are findings, not automatic exemptions.

Capture state before issuing the operation, not only after the first amendment.
For failures, check every relevant state dimension even if the returned outcome
is already wrong. Use a structured comparison or collect observations before
asserting so one early assertion does not hide all remaining evidence.

## Step 3: Add Focused Cases Incrementally

These IDs are proposed evidence labels, not existing Rust test names. Choose names
consistent with the owning tests. Use a fresh fixture copy for each case.

### A1: Later Rejection After An Applicable Amendment

Create two independently selected changes with justified mutable targets. Make the
first applicable and the later one rejected by a deterministic existing behavior.
Use W01's verified path to choose that blocker; do not assume invalid coordinates
will survive input validation long enough to exercise late application.

Demonstrate in a fresh control fixture that the first change succeeds alone.
Then run the combined invocation. Expected result: explicit non-success, no
selected changes absorbed, and the state oracle unchanged. On current code,
record any moved ref or changed commit blob even if rejection is returned as a
count instead of an error. Do not require a future error type that does not exist.

If preflight rejects before the first amendment, that is a useful passing safety
case but not proof of late-rejection rollback. Name the actual reached boundary.

### A2: Cross-Branch Failure

Place one applicable selection on each of two independent applied branches, with
a deterministic blocker on one. Ensure dependency evidence is unambiguous. A
failure on either branch must prevent publication on both, preserving all relevant
refs and metadata. Verify that no eligible subset is reported as complete success.
Use ordering controls only if they are supported inputs; do not depend on incidental
map iteration order. Record whether this exercises preflight or late execution.

### A3: Planning-Created Blank Commit Followed By Failure

Use the audited planner path that creates a blank commit for a selected change,
then combine it with a blocked selection. Capture state before planning. On
failure, no new reachable blank commit, ref change, metadata record, assignment,
or visible operation entry may remain. Merely restoring the amended target is
insufficient. Run through the actual planner, not a manually fabricated blank
commit that bypasses the behavior under test.

### A4: Selected Immutable Or Landed Target

Pair an eligible selected change with an explicitly selected ineligible target.
Cover immutable and landed classifications separately if W01 identifies distinct
handling paths. The entire invocation must fail unchanged; silently dropping the
ineligible selection and applying the rest is not success.

Test at the caller that performs filtering, including the CLI if necessary.
Establish that the ineligible change really was selected before filtering. Do
not confuse an unselected landed commit in history with a selected landed target.

### A5: Successful Invocation Counterparts

Remove the blocker from representative same-branch and cross-branch fixtures.
Assert every selected change is committed exactly once into its intended target,
descendants and refs are consistent, worktree content is preserved, and unselected
changes remain as the exact residual diff. Check index, metadata, assignments,
and oplog against the audited success contract, not the failure equality oracle.

A success case must prevent an implementation that achieves atomicity by rejecting
everything from satisfying this suite. Do not invent a new oplog entry-count
contract here; unresolved success bookkeeping belongs in W04.

### A6: Dry-Run Counterparts

Exercise the real dry-run entry point on a valid invocation and on a blocked one.
Include a planning-created-commit case because mutation can precede the dry-run
check. From before planning through return, persistent semantic state must remain
unchanged for both outcomes. Check the existing preview contract without assuming
a new result shape or treating a successful preview as permission to publish.

## Step 4: Run And Classify Evidence

Immediately after each substantive test edit, run the new named test. Discover
the real name with the existing test listing; verify a nonzero selected count.
Representative commands, with placeholders replaced by actual names:

```sh
cargo test -p but-api <actual_api_test_name> -- --nocapture
cargo test -p but --test but <actual_cli_test_name> -- --nocapture
```

Compilation, fixture setup, or environment errors do not establish atomicity
failures. Repair local test defects and rerun the same focused check. Record exact
observed state changes and distinguish a returned rejection from actual unchanged
state. Keep expected red tests executable and clearly identified at handoff.

Run the affected existing absorb group for comparison after the new cases. Use
the W00 baseline to distinguish existing failures from new policy assertions.
Run scoped formatting/lint checks and fixture syntax checks prescribed by the
owning instructions. Avoid an unrelated full-workspace suite.

Label newly failing atomicity tests as approved policy changes unless independent
historical evidence establishes a regression introduced by the PR. W02's selector
defects and W03's pre-existing partial behavior have different attribution.

## Step 5: Publication Errors And Deferred Coverage

List audited failure points before publication, during materialization, and after
publication, including metadata/database persistence and oplog finalization.
For each, record existing injection support, what can be observed, and the packet
responsible for further testing. Exercise an existing suitable test hook when
available; do not build production injection machinery in this packet.

A returned `Err` after publication does not prove rollback. Flag uncertain or
failed restoration explicitly and retain it as a W04/W07 release concern. Do not
claim crash-proof ACID behavior, enter conflict mode automatically, or invent a
recovery command. Recovery/finalization gaps do not disappear because ordinary
rejection tests pass.

## Step 6: Hand Off And Stop

For each case record the test name, fixture, source identity, command, nonzero
test count, outcome, tested operation boundary, and expected/actual state delta.
Include the applicable-alone control and successful/dry-run counterparts. Record
which oracle dimensions were verified, not applicable with rationale, or blocked.

W03 is complete when the required policy cases have executable evidence, ordinary
failure invariants are explicit, and finalization gaps have named follow-up owners.
Missing essential cases or an unavailable state oracle require a precise blocker,
not a completion claim. Red policy tests can complete W03; they cannot clear the
implementation or release gate.

Update the ledger, clear ownership, and choose the next ready packet: W02 if still
pending, otherwise W04 for technical design review. Do not mark W04 approved or
start W05. Validate modified documentation and stop with a concise handoff listing
all expected failing tests and any unverified state dimensions.

## Prompt To Assign This Packet

> Execute absorb-w03-atomicity-tests.md only after verifying W01 evidence. Add
> fixture-backed atomicity tests covering late rejection, independent branches,
> planning-created commits, selected ineligible targets, success, and dry-run.
> Compare semantic state from before planning through return and record exact
> evidence in absorb-verification-ledger.md. Do not fix production code, suppress
> failures, commit, push, switch branches, or update GitHub. Stop after W03 with
> named test results and explicit finalization or state-oracle gaps.
